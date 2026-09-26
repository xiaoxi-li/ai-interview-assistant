//! 隐私设置：音频是否上传、本地数据加密开关与数据清理。
//!
//! 说明：`upload_audio` 与 ASR 模式相互约束——云端 ASR 必然需要上传音频；
//! 本地模式下该开关无实际作用，UI 会据此禁用并提示。

use super::{load_setting, save_setting};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// settings 表中隐私配置的键名
pub const SETTING_PRIVACY_CONFIG: &str = "privacy.config";

/// 隐私配置
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyConfig {
    /// 是否允许将音频上传至云端 ASR
    pub upload_audio: bool,
    /// 是否启用本地敏感数据加密（转写与问答内容）
    pub encrypt_local: bool,
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            // 方案确认：云端 ASR 需用户显式开启，因此默认关闭
            upload_audio: false,
            encrypt_local: false,
        }
    }
}

impl PrivacyConfig {
    /// 结合 ASR 模式校验：本地模式下不应开启上传
    pub fn validate(&self, cloud_asr_enabled: bool) -> Result<(), String> {
        if self.upload_audio && !cloud_asr_enabled {
            return Err("本地 ASR 模式下无需上传音频，请先切换为云端模式".into());
        }
        Ok(())
    }
}

/// 读取隐私配置；缺失或解析失败时回退默认值
pub fn load_privacy_config(conn: &Connection) -> rusqlite::Result<PrivacyConfig> {
    let Some(raw) = load_setting(conn, SETTING_PRIVACY_CONFIG)? else {
        return Ok(PrivacyConfig::default());
    };
    match serde_json::from_str::<PrivacyConfig>(&raw) {
        Ok(config) => Ok(config),
        Err(err) => {
            eprintln!("隐私配置解析失败，回退默认值：{err}");
            Ok(PrivacyConfig::default())
        }
    }
}

/// 保存隐私配置
pub fn save_privacy_config(conn: &Connection, config: &PrivacyConfig) -> Result<(), String> {
    let json = serde_json::to_string(config).map_err(|e| e.to_string())?;
    save_setting(conn, SETTING_PRIVACY_CONFIG, &json).map_err(|e| e.to_string())
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
    fn default_privacy_is_most_conservative() {
        let config = PrivacyConfig::default();
        assert!(!config.upload_audio, "默认不上传音频");
        assert!(!config.encrypt_local);
    }

    #[test]
    fn privacy_config_round_trip() {
        with_db(|conn| {
            let config = PrivacyConfig {
                upload_audio: true,
                encrypt_local: true,
            };
            save_privacy_config(conn, &config).expect("save");
            assert_eq!(load_privacy_config(conn).expect("load"), config);
        });
    }

    #[test]
    fn missing_or_corrupted_config_falls_back() {
        with_db(|conn| {
            assert_eq!(
                load_privacy_config(conn).expect("load"),
                PrivacyConfig::default()
            );
            save_setting(conn, SETTING_PRIVACY_CONFIG, "oops").expect("save raw");
            assert_eq!(
                load_privacy_config(conn).expect("load"),
                PrivacyConfig::default()
            );
        });
    }

    #[test]
    fn upload_requires_cloud_asr() {
        let config = PrivacyConfig {
            upload_audio: true,
            encrypt_local: false,
        };
        assert!(config.validate(true).is_ok());
        assert!(config.validate(false).is_err());
    }

    #[test]
    fn local_mode_without_upload_is_valid() {
        let config = PrivacyConfig::default();
        assert!(config.validate(false).is_ok());
    }
}
