//! 音频设备配置：麦克风与系统音频来源选择。

use super::{load_setting, save_setting};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// settings 表中音频配置的键名
pub const SETTING_AUDIO_CONFIG: &str = "audio.config";

/// 音频设备配置
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioConfig {
    /// 麦克风设备 ID（None 表示使用系统默认）
    pub mic_device_id: Option<String>,
    /// 系统音频输出设备 ID（None 表示使用系统默认）
    pub system_device_id: Option<String>,
    /// 是否启用回声消除（IA-02/IA-05）：以系统音频流为参考消除麦克风中的面试官回声
    #[serde(default = "default_true")]
    pub aec_enabled: bool,
    /// 进程级回环目标进程 PID（TODO-05 / IA-14）：`Some(pid)` 时「面试官」通道只采集
    /// 该进程树（如会议软件）的音频，避免背景音乐等一并混入；
    /// `None` 时为端点级回环（采集整个播放设备的混音）。仅 Windows 10 2004+ 生效。
    #[serde(default)]
    pub process_loopback_pid: Option<u32>,
}

/// 新增字段默认值（兼容旧配置文件）
fn default_true() -> bool {
    true
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            mic_device_id: None,
            system_device_id: None,
            aec_enabled: true,
            process_loopback_pid: None,
        }
    }
}

impl AudioConfig {
    /// 校验：显式选择的设备 ID 不能是空白字符串
    pub fn validate(&self) -> Result<(), String> {
        if let Some(id) = &self.mic_device_id {
            if id.trim().is_empty() {
                return Err("麦克风设备 ID 不能为空白".into());
            }
        }
        if let Some(id) = &self.system_device_id {
            if id.trim().is_empty() {
                return Err("系统音频设备 ID 不能为空白".into());
            }
        }
        Ok(())
    }

    /// 是否使用系统默认设备
    pub fn uses_defaults(&self) -> bool {
        self.mic_device_id.is_none()
            && self.system_device_id.is_none()
            && self.process_loopback_pid.is_none()
    }
}

/// 读取音频配置；缺失或解析失败时回退默认值
pub fn load_audio_config(conn: &Connection) -> rusqlite::Result<AudioConfig> {
    let Some(raw) = load_setting(conn, SETTING_AUDIO_CONFIG)? else {
        return Ok(AudioConfig::default());
    };
    match serde_json::from_str::<AudioConfig>(&raw) {
        Ok(config) => Ok(config),
        Err(err) => {
            tracing::warn!("音频配置解析失败，回退默认值：{err}");
            Ok(AudioConfig::default())
        }
    }
}

/// 保存音频配置
pub fn save_audio_config(conn: &Connection, config: &AudioConfig) -> Result<(), String> {
    config.validate()?;
    let json = serde_json::to_string(config).map_err(|e| e.to_string())?;
    save_setting(conn, SETTING_AUDIO_CONFIG, &json).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Db;

    fn with_db<T>(f: impl FnOnce(&Connection) -> T) -> T {
        let db = Db::open_in_memory().expect("db");
        f(db.conn())
    }

    #[test]
    fn default_uses_system_devices() {
        let config = AudioConfig::default();
        assert!(config.uses_defaults());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn audio_config_round_trip() {
        with_db(|conn| {
            let config = AudioConfig {
                mic_device_id: Some("麦克风 (USB)".into()),
                system_device_id: Some("扬声器 (Realtek)".into()),
                aec_enabled: false,
                process_loopback_pid: Some(12_345),
            };
            save_audio_config(conn, &config).expect("save");
            assert_eq!(load_audio_config(conn).expect("load"), config);
        });
    }

    #[test]
    fn legacy_config_without_new_fields_parses() {
        with_db(|conn| {
            save_setting(
                conn,
                SETTING_AUDIO_CONFIG,
                r#"{"micDeviceId":null,"systemDeviceId":null}"#,
            )
            .expect("save legacy");
            let loaded = load_audio_config(conn).expect("load");
            assert!(loaded.aec_enabled, "旧配置 AEC 默认开启");
            assert_eq!(
                loaded.process_loopback_pid, None,
                "旧配置进程回环目标默认为空（端点级回环）"
            );
        });
    }

    #[test]
    fn process_loopback_config_round_trip() {
        with_db(|conn| {
            let config = AudioConfig {
                process_loopback_pid: Some(9_999),
                ..AudioConfig::default()
            };
            save_audio_config(conn, &config).expect("save");
            assert_eq!(load_audio_config(conn).expect("load"), config);
        });
    }

    #[test]
    fn missing_config_falls_back_to_default() {
        with_db(|conn| {
            assert_eq!(
                load_audio_config(conn).expect("load"),
                AudioConfig::default()
            );
        });
    }

    #[test]
    fn corrupted_config_falls_back_to_default() {
        with_db(|conn| {
            save_setting(conn, SETTING_AUDIO_CONFIG, "not-json").expect("save raw");
            assert_eq!(
                load_audio_config(conn).expect("load"),
                AudioConfig::default()
            );
        });
    }

    #[test]
    fn validate_rejects_blank_ids() {
        let mut config = AudioConfig::default();
        config.mic_device_id = Some("   ".into());
        assert!(config.validate().is_err());

        config.mic_device_id = None;
        config.system_device_id = Some(String::new());
        assert!(config.validate().is_err());

        config.system_device_id = Some("扬声器".into());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn save_rejects_invalid_config() {
        with_db(|conn| {
            let config = AudioConfig {
                mic_device_id: Some(" ".into()),
                system_device_id: None,
                aec_enabled: true,
                process_loopback_pid: None,
            };
            assert!(save_audio_config(conn, &config).is_err());
        });
    }
}
