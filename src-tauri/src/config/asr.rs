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

/// 云端 ASR 服务商。
///
/// `AsrConfig::cloud_base_urls` 以其为 map 键，故需 `Ord`；序列化名保持稳定
/// （`openaicompat` 为历史标识，`openai_compatible` 为规范名）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CloudAsrProvider {
    #[serde(rename = "xfyun")]
    Xfyun,
    #[serde(rename = "aliyun")]
    Aliyun,
    #[serde(rename = "tencent")]
    Tencent,
    /// OpenAI 兼容（自定义）：HTTP 转写接口，Base URL 必填
    #[serde(rename = "openai_compatible", alias = "openaicompat")]
    OpenaiCompat,
}

impl CloudAsrProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            CloudAsrProvider::Xfyun => "xfyun",
            CloudAsrProvider::Aliyun => "aliyun",
            CloudAsrProvider::Tencent => "tencent",
            CloudAsrProvider::OpenaiCompat => "openai_compatible",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "xfyun" | "iflytek" => Some(CloudAsrProvider::Xfyun),
            "aliyun" | "dashscope" => Some(CloudAsrProvider::Aliyun),
            "tencent" => Some(CloudAsrProvider::Tencent),
            "openai_compatible" | "openaicompat" | "openai" => {
                Some(CloudAsrProvider::OpenaiCompat)
            }
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            CloudAsrProvider::Xfyun => "讯飞开放平台",
            CloudAsrProvider::Aliyun => "阿里云智能语音",
            CloudAsrProvider::Tencent => "腾讯云语音识别",
            CloudAsrProvider::OpenaiCompat => "OpenAI 兼容（自定义）",
        }
    }

    /// 默认流式模型
    ///
    /// 阿里云默认取 `qwen-audio-3.1-asr-flash-streaming`：官方推荐的实时识别模型
    /// （Gummy 系已标注「即将下线」）。已实测可用，备选：`fun-asr-realtime`、
    /// `paraformer-realtime-v2`。注意 **`qwen-audio-3.1-asr-flash`（无 `-streaming`）
    /// 是非实时 HTTP 模型**，填到实时端点会被拒（ModelNotFound）。
    pub fn default_model(self) -> &'static str {
        match self {
            CloudAsrProvider::Xfyun => "iat",
            CloudAsrProvider::Aliyun => "qwen-audio-3.1-asr-flash-streaming",
            CloudAsrProvider::Tencent => "16k_zh",
            CloudAsrProvider::OpenaiCompat => "whisper-1",
        }
    }

    /// 官方默认服务端地址（Base URL），可被用户在设置页覆盖
    pub fn default_base_url(self) -> &'static str {
        match self {
            CloudAsrProvider::Xfyun => "wss://ws-api.xfyun.cn/v2/iat",
            CloudAsrProvider::Aliyun => "wss://dashscope.aliyuncs.com/api-ws/v1/inference",
            CloudAsrProvider::Tencent => "wss://asr.cloud.tencent.com/asr/v2",
            CloudAsrProvider::OpenaiCompat => "https://api.openai.com/v1",
        }
    }

    /// 该服务商是否走 WebSocket 实时识别（否则为 HTTP 转写接口）
    pub fn uses_websocket(self) -> bool {
        !matches!(self, CloudAsrProvider::OpenaiCompat)
    }

    /// 协议适配是否已实现（未实现者由命令层给出**可操作错误**，不静默失败）
    pub fn implemented(self) -> bool {
        matches!(self, CloudAsrProvider::Aliyun)
    }

    /// 该服务商需要的凭证字段
    pub fn credential_fields(self) -> &'static [&'static str] {
        match self {
            // 讯飞 WebSocket 鉴权需要三元组
            CloudAsrProvider::Xfyun => &["app_id", "api_key", "api_secret"],
            CloudAsrProvider::Aliyun => &["api_key"],
            CloudAsrProvider::Tencent => &["secret_id", "secret_key"],
            CloudAsrProvider::OpenaiCompat => &["api_key"],
        }
    }
}

/// 全部云端服务商
pub fn all_cloud_providers() -> Vec<CloudAsrProvider> {
    vec![
        CloudAsrProvider::Aliyun,
        CloudAsrProvider::OpenaiCompat,
        CloudAsrProvider::Xfyun,
        CloudAsrProvider::Tencent,
    ]
}

/// 界面展示顺序（阿里云优先：当前已接入）
pub fn cloud_providers_in_display_order() -> Vec<CloudAsrProvider> {
    vec![
        CloudAsrProvider::Aliyun,
        CloudAsrProvider::Xfyun,
        CloudAsrProvider::Tencent,
        CloudAsrProvider::OpenaiCompat,
    ]
}

/// 本地 ASR 默认模型（sherpa-onnx 中文 paraformer）
pub const DEFAULT_LOCAL_MODEL: &str = "sherpa-onnx-paraformer-zh";

/// Base URL 允许的协议前缀（ws(s) 用于实时识别，http(s) 用于 OpenAI 兼容转写/反向代理）
const BASE_URL_SCHEMES: [&str; 4] = ["ws://", "wss://", "http://", "https://"];

/// 校验单个服务商的自定义 Base URL（仅在用户填写了值时调用）
pub fn validate_base_url(value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("服务端地址不能为空（如需官方默认，请恢复默认或清空该服务商的自定义值）".into());
    }
    if !BASE_URL_SCHEMES
        .iter()
        .any(|scheme| trimmed.starts_with(scheme))
    {
        return Err("服务端地址需以 ws://、wss://、http:// 或 https:// 开头".into());
    }
    Ok(())
}

/// 把 `http(s)://` 地址改写为 `ws(s)://`（兼容反向代理 / 网关写法）
pub fn to_websocket_url(value: &str) -> String {
    let trimmed = value.trim();
    if let Some(rest) = trimmed.strip_prefix("https://") {
        return format!("wss://{rest}");
    }
    if let Some(rest) = trimmed.strip_prefix("http://") {
        return format!("ws://{rest}");
    }
    trimmed.to_string()
}

/// 协议尚未接入时的**可操作错误文案**（命令层与云端工厂共用）
pub fn unsupported_provider_message(provider: CloudAsrProvider) -> String {
    format!(
        "「{}」的云端 ASR 待接入，当前仅「阿里云智能语音」已接入。请到「设置 → 语音识别」切换服务商，或改用「本地（隐私优先）」后再开始转写。",
        provider.label()
    )
}

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
    /// 各服务商自定义服务端地址（Base URL），**每个服务商各存一份**；缺失或空串表示使用官方默认。
    /// 向后兼容：旧配置无此字段时按空表解析。
    #[serde(default)]
    pub cloud_base_urls: std::collections::BTreeMap<CloudAsrProvider, String>,
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
            cloud_base_urls: std::collections::BTreeMap::new(),
        }
    }

    /// 当前模式实际使用的模型名
    pub fn active_model(&self) -> &str {
        match self.mode {
            AsrMode::Cloud => &self.model,
            AsrMode::Local => &self.local_model,
        }
    }

    /// 读取某服务商生效的服务端地址：自定义值优先，否则回退官方默认
    pub fn base_url_for(&self, provider: CloudAsrProvider) -> String {
        self.cloud_base_urls
            .get(&provider)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string())
            .unwrap_or_else(|| provider.default_base_url().to_string())
    }

    /// 该服务商是否使用了自定义地址（与官方默认不同）
    pub fn has_custom_base_url(&self, provider: CloudAsrProvider) -> bool {
        self.cloud_base_urls
            .get(&provider)
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false)
    }

    /// 设置某服务商的自定义地址；空白值表示恢复官方默认（移除自定义项）
    pub fn set_base_url(&mut self, provider: CloudAsrProvider, value: &str) {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            self.cloud_base_urls.remove(&provider);
        } else {
            self.cloud_base_urls
                .insert(provider, trimmed.to_string());
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
        // 各服务商的自定义 Base URL：仅校验已填写的项（空串等价于使用官方默认）
        for (provider, value) in &self.cloud_base_urls {
            if value.trim().is_empty() {
                continue;
            }
            validate_base_url(value).map_err(|err| format!("{}：{err}", provider.label()))?;
        }
        Ok(())
    }
}

impl Default for AsrConfig {
    fn default() -> Self {
        // 阶段性决策：P0 只做本地 ASR 通路，默认取本地模式
        Self::from_mode(AsrMode::Local)
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
            tracing::warn!("ASR 配置解析失败，回退默认值：{err}");
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
    fn default_config_targets_local_with_zh_language() {
        let config = AsrConfig::default();
        assert_eq!(config.mode, AsrMode::Local, "P0 默认走本地 ASR 通路");
        assert_eq!(config.cloud_provider, CloudAsrProvider::Xfyun);
        assert_eq!(config.language, "zh-CN");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn only_aliyun_cloud_protocol_is_implemented() {
        // P1：阿里云百炼已接入；其余服务商保持「待接入」并由命令层给出可操作错误
        assert!(CloudAsrProvider::Aliyun.implemented());
        for provider in [
            CloudAsrProvider::Xfyun,
            CloudAsrProvider::Tencent,
            CloudAsrProvider::OpenaiCompat,
        ] {
            assert!(!provider.implemented(), "{} 尚未接入", provider.as_str());
        }
    }

    #[test]
    fn provider_defaults_cover_base_urls_and_transport() {
        for provider in all_cloud_providers() {
            let base_url = provider.default_base_url();
            assert!(
                ["ws://", "wss://", "http://", "https://"]
                    .iter()
                    .any(|scheme| base_url.starts_with(scheme)),
                "{} 默认地址不合法：{base_url}",
                provider.as_str()
            );
            assert!(!provider.default_model().is_empty());
            assert!(!provider.credential_fields().is_empty());
        }
        assert!(CloudAsrProvider::Aliyun.uses_websocket());
        assert!(!CloudAsrProvider::OpenaiCompat.uses_websocket());
    }

    #[test]
    fn openai_compatible_provider_parses_and_serializes_as_map_key() {
        assert_eq!(
            CloudAsrProvider::parse("openai_compatible"),
            Some(CloudAsrProvider::OpenaiCompat)
        );
        assert_eq!(
            CloudAsrProvider::parse("OpenAIChatGPT".to_ascii_lowercase().as_str()),
            None
        );
        assert_eq!(
            CloudAsrProvider::parse("openaicompat"),
            Some(CloudAsrProvider::OpenaiCompat)
        );
        assert_eq!(CloudAsrProvider::OpenaiCompat.as_str(), "openai_compatible");
        // 枚举作 map 键时必须序列化为字符串（serde_json 要求）
        let json = serde_json::to_string(&CloudAsrProvider::OpenaiCompat).expect("serialize");
        assert_eq!(json, "\"openai_compatible\"");
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
        assert_eq!(all_cloud_providers().len(), 4);
        assert_eq!(
            cloud_providers_in_display_order().len(),
            4,
            "展示顺序应覆盖全部服务商"
        );
        assert_eq!(
            cloud_providers_in_display_order()[0],
            CloudAsrProvider::Aliyun,
            "已接入的服务商排在最前"
        );
    }

    #[test]
    fn active_model_follows_mode() {
        let mut config = AsrConfig::default();
        // P0 默认本地模式
        assert_eq!(config.active_model(), DEFAULT_LOCAL_MODEL);

        config.mode = AsrMode::Cloud;
        assert_eq!(config.active_model(), "iat");
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
                cloud_base_urls: [
                    (
                        CloudAsrProvider::Aliyun,
                        "wss://proxy.example.com/dashscope".to_string(),
                    ),
                    (
                        CloudAsrProvider::OpenaiCompat,
                        "https://asr.example.com/v1".to_string(),
                    ),
                ]
                .into_iter()
                .collect(),
            };
            save_asr_config(conn, &config).expect("save");
            assert_eq!(load_asr_config(conn).expect("load"), config);
        });
    }

    #[test]
    fn base_url_falls_back_to_official_default_and_can_be_overridden() {
        let mut config = AsrConfig::default();
        assert_eq!(
            config.base_url_for(CloudAsrProvider::Aliyun),
            CloudAsrProvider::Aliyun.default_base_url()
        );
        assert!(!config.has_custom_base_url(CloudAsrProvider::Aliyun));

        config.set_base_url(CloudAsrProvider::Aliyun, "  wss://proxy.example.com/ws  ");
        assert_eq!(
            config.base_url_for(CloudAsrProvider::Aliyun),
            "wss://proxy.example.com/ws",
            "自定义值应被 trim 后使用"
        );
        assert!(config.has_custom_base_url(CloudAsrProvider::Aliyun));

        // 每个服务商各存一份：覆盖阿里不影响腾讯
        config.set_base_url(CloudAsrProvider::Tencent, "wss://t.example.com/asr");
        assert_eq!(
            config.base_url_for(CloudAsrProvider::Tencent),
            "wss://t.example.com/asr"
        );

        // 空白值等价于恢复官方默认
        config.set_base_url(CloudAsrProvider::Aliyun, "   ");
        assert_eq!(
            config.base_url_for(CloudAsrProvider::Aliyun),
            CloudAsrProvider::Aliyun.default_base_url()
        );
        assert!(!config.has_custom_base_url(CloudAsrProvider::Aliyun));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validate_rejects_invalid_base_url_scheme() {
        let mut config = AsrConfig::default();
        config.set_base_url(CloudAsrProvider::Aliyun, "ftp://example.com");
        let err = config.validate().expect_err("非 ws/http scheme 应被拒绝");
        assert!(err.contains("阿里云智能语音"), "错误文案应指明服务商：{err}");

        config.set_base_url(CloudAsrProvider::Aliyun, "wss://example.com/ws");
        assert!(config.validate().is_ok());
        config.set_base_url(CloudAsrProvider::Aliyun, "http://example.com/ws");
        assert!(
            config.validate().is_ok(),
            "http(s) 为反向代理写法，应被接受（连接前改写为 ws(s)）"
        );
    }

    #[test]
    fn to_websocket_url_rewrites_http_scheme() {
        assert_eq!(
            to_websocket_url("https://example.com/ws"),
            "wss://example.com/ws"
        );
        assert_eq!(
            to_websocket_url("http://example.com/ws"),
            "ws://example.com/ws"
        );
        assert_eq!(
            to_websocket_url("wss://example.com/ws"),
            "wss://example.com/ws"
        );
    }

    #[test]
    fn legacy_config_without_base_urls_still_parses() {
        with_db(|conn| {
            // 旧版配置：无 cloudBaseUrls 字段
            let legacy = serde_json::json!({
                "mode": "cloud",
                "cloudProvider": "aliyun",
                "model": "gummy-realtime-v1",
                "language": "zh-CN",
                "vadThreshold": 0.5,
                "localModel": DEFAULT_LOCAL_MODEL,
            });
            save_setting(conn, SETTING_ASR_CONFIG, &legacy.to_string()).expect("save raw");

            let loaded = load_asr_config(conn).expect("load");
            assert_eq!(loaded.cloud_provider, CloudAsrProvider::Aliyun);
            assert!(loaded.cloud_base_urls.is_empty(), "缺失字段应按空表解析");
            assert_eq!(
                loaded.base_url_for(CloudAsrProvider::Aliyun),
                CloudAsrProvider::Aliyun.default_base_url()
            );
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
