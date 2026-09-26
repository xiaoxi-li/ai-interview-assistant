//! ASR 配置：云端/本地模式、云端服务商、模型、语言与 VAD 阈值。
//!
//! 服务商凭证（AppID / API Key / API Secret）不入库，统一由系统钥匙链保存，
//! 通过 [`crate::security::asr_credential_ref`] 生成存储键。

use super::{load_setting, save_setting};
use crate::providers::types::AsrMode;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// settings 表中 ASR 配置的键名
pub const SETTING_ASR_CONFIG: &str = "asr.config";

/// 云端 ASR 服务商（方案确认：首期讯飞，阿里/腾讯后续接入）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CloudAsrProvider {
    Xfyun,
    Aliyun,
    Tencent,
}

impl CloudAsrProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            CloudAsrProvider::Xfyun => "xfyun",
            CloudAsrProvider::Aliyun => "aliyun",
            CloudAsrProvider::Tencent => "tencent",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "xfyun" | "iflytek" => Some(CloudAsrProvider::Xfyun),
            "aliyun" | "dashscope" => Some(CloudAsrProvider::Aliyun),
            "tencent" => Some(CloudAsrProvider::Tencent),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            CloudAsrProvider::Xfyun => "讯飞开放平台",
            CloudAsrProvider::Aliyun => "阿里云智能语音",
            CloudAsrProvider::Tencent => "腾讯云语音识别",
        }
    }

    /// 默认流式模型
    pub fn default_model(self) -> &'static str {
        match self {
            CloudAsrProvider::Xfyun => "iat",
            CloudAsrProvider::Aliyun => "gummy-realtime-v1",
            CloudAsrProvider::Tencent => "16k_zh",
        }
    }

    /// 该服务商需要的凭证字段
    pub fn credential_fields(self) -> &'static [&'static str] {
        match self {
            // 讯飞 WebSocket 鉴权需要三元组
            CloudAsrProvider::Xfyun => &["app_id", "api_key", "api_secret"],
            CloudAsrProvider::Aliyun => &["api_key"],
            CloudAsrProvider::Tencent => &["secret_id", "secret_key"],
        }
    }
}

/// 全部云端服务商
pub fn all_cloud_providers() -> Vec<CloudAsrProvider> {
    vec![
        CloudAsrProvider::Xfyun,
        CloudAsrProvider::Aliyun,
        CloudAsrProvider::Tencent,
    ]
}

/// 本地 ASR 默认模型（sherpa-onnx 中文 paraformer）
pub const DEFAULT_LOCAL_MODEL: &str = "sherpa-onnx-paraformer-zh";

/// VAD 阈值合法区间
pub const VAD_THRESHOLD_MIN: f32 = 0.0;
pub const VAD_THRESHOLD_MAX: f32 = 1.0;

/// ASR 配置（不含凭证）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrConfig {
    pub mode: AsrMode,
    pub cloud_provider: CloudAsrProvider,
    pub model: String,
    pub language: String,
    /// VAD 判定阈值（0~1，越大越严格）
    pub vad_threshold: f32,
    pub local_model: String,
}

impl AsrConfig {
    /// 按运行模式构造默认配置
    pub fn from_mode(mode: AsrMode) -> Self {
        Self {
            mode,
            cloud_provider: CloudAsrProvider::Xfyun,
            model: CloudAsrProvider::Xfyun.default_model().to_string(),
            language: "zh-CN".to_string(),
            vad_threshold: 0.5,
            local_model: DEFAULT_LOCAL_MODEL.to_string(),
        }
    }

    /// 当前模式实际使用的模型名
    pub fn active_model(&self) -> &str {
        match self.mode {
            AsrMode::Cloud => &self.model,
            AsrMode::Local => &self.local_model,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.model.trim().is_empty() {
            return Err("云端模型名不能为空".into());
        }
        if self.local_model.trim().is_empty() {
            return Err("本地模型名不能为空".into());
        }
        if self.language.trim().is_empty() {
            return Err("识别语言不能为空".into());
        }
        if !(VAD_THRESHOLD_MIN..=VAD_THRESHOLD_MAX).contains(&self.vad_threshold) {
            return Err(format!(
                "VAD 阈值需在 {VAD_THRESHOLD_MIN} ~ {VAD_THRESHOLD_MAX} 之间"
            ));
        }
        Ok(())
    }
}

impl Default for AsrConfig {
    fn default() -> Self {
        // 方案确认：首次启动向导显式选择，默认取云端（向导会覆盖）
        Self::from_mode(AsrMode::Cloud)
    }
}

/// 读取 ASR 配置；缺失或解析失败时回退默认值
pub fn load_asr_config(conn: &Connection) -> rusqlite::Result<AsrConfig> {
    let Some(raw) = load_setting(conn, SETTING_ASR_CONFIG)? else {
        return Ok(AsrConfig::default());
    };
    match serde_json::from_str::<AsrConfig>(&raw) {
        Ok(config) => Ok(config),
        Err(err) => {
            eprintln!("ASR 配置解析失败，回退默认值：{err}");
            Ok(AsrConfig::default())
        }
    }
}

/// 保存 ASR 配置
pub fn save_asr_config(conn: &Connection, config: &AsrConfig) -> Result<(), String> {
    config.validate()?;
    let json = serde_json::to_string(config).map_err(|e| e.to_string())?;
    save_setting(conn, SETTING_ASR_CONFIG, &json).map_err(|e| e.to_string())
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
    fn default_config_targets_cloud_with_zh_language() {
        let config = AsrConfig::default();
        assert_eq!(config.mode, AsrMode::Cloud);
        assert_eq!(config.cloud_provider, CloudAsrProvider::Xfyun);
        assert_eq!(config.language, "zh-CN");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn provider_parse_and_presets() {
        assert_eq!(
            CloudAsrProvider::parse("XFYUN"),
            Some(CloudAsrProvider::Xfyun)
        );
        assert_eq!(
            CloudAsrProvider::parse("dashscope"),
            Some(CloudAsrProvider::Aliyun)
        );
        assert_eq!(CloudAsrProvider::parse("unknown"), None);

        assert_eq!(
            CloudAsrProvider::Xfyun.credential_fields(),
            &["app_id", "api_key", "api_secret"]
        );
        assert_eq!(all_cloud_providers().len(), 3);
    }

    #[test]
    fn active_model_follows_mode() {
        let mut config = AsrConfig::default();
        assert_eq!(config.active_model(), "iat");

        config.mode = AsrMode::Local;
        assert_eq!(config.active_model(), DEFAULT_LOCAL_MODEL);
    }

    #[test]
    fn asr_config_round_trip() {
        with_db(|conn| {
            let config = AsrConfig {
                mode: AsrMode::Local,
                cloud_provider: CloudAsrProvider::Tencent,
                model: "16k_zh".into(),
                language: "zh-CN".into(),
                vad_threshold: 0.35,
                local_model: DEFAULT_LOCAL_MODEL.into(),
            };
            save_asr_config(conn, &config).expect("save");
            assert_eq!(load_asr_config(conn).expect("load"), config);
        });
    }

    #[test]
    fn missing_or_corrupted_config_falls_back() {
        with_db(|conn| {
            assert_eq!(load_asr_config(conn).expect("load"), AsrConfig::default());
            save_setting(conn, SETTING_ASR_CONFIG, "{ broken").expect("save raw");
            assert_eq!(load_asr_config(conn).expect("load"), AsrConfig::default());
        });
    }

    #[test]
    fn validate_rejects_invalid_fields() {
        let mut config = AsrConfig::default();

        config.model = " ".into();
        assert!(config.validate().is_err());

        config.model = "iat".into();
        config.local_model = "".into();
        assert!(config.validate().is_err());

        config.local_model = DEFAULT_LOCAL_MODEL.into();
        config.language = "".into();
        assert!(config.validate().is_err());

        config.language = "zh-CN".into();
        config.vad_threshold = 1.5;
        assert!(config.validate().is_err());

        config.vad_threshold = 0.0;
        assert!(config.validate().is_ok());
        config.vad_threshold = 1.0;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn save_rejects_invalid_config() {
        with_db(|conn| {
            let mut config = AsrConfig::default();
            config.vad_threshold = -0.1;
            assert!(save_asr_config(conn, &config).is_err());
        });
    }
}
