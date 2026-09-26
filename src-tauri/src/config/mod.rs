//! 应用配置：AI 平台配置与通用设置项的持久化。
//!
//! 配置以 JSON 形式存放于 `settings` 表，避免为每个字段新增列；
//! 敏感字段（API Key）不落库，由 [`crate::security`] 存入系统钥匙链。

pub mod asr;
pub mod audio;
pub mod privacy;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// settings 表中 AI 配置的键名
pub const SETTING_AI_CONFIG: &str = "ai.config";

/// 支持的 AI 平台
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AiPlatform {
    Deepseek,
    Qwen,
    Glm,
    Chatgpt,
}

impl AiPlatform {
    pub fn as_str(self) -> &'static str {
        match self {
            AiPlatform::Deepseek => "deepseek",
            AiPlatform::Qwen => "qwen",
            AiPlatform::Glm => "glm",
            AiPlatform::Chatgpt => "chatgpt",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "deepseek" => Some(AiPlatform::Deepseek),
            "qwen" => Some(AiPlatform::Qwen),
            "glm" => Some(AiPlatform::Glm),
            "chatgpt" | "openai" => Some(AiPlatform::Chatgpt),
            _ => None,
        }
    }

    /// 平台预设（Base URL 与默认模型）
    pub fn preset(self) -> PlatformPreset {
        let (label, base_url, model) = match self {
            AiPlatform::Deepseek => ("DeepSeek", "https://api.deepseek.com/v1", "deepseek-chat"),
            AiPlatform::Qwen => (
                "通义千问 Qwen",
                "https://dashscope.aliyuncs.com/compatible-mode/v1",
                "qwen-plus",
            ),
            AiPlatform::Glm => (
                "智谱 GLM",
                "https://open.bigmodel.cn/api/paas/v4",
                "glm-4-plus",
            ),
            AiPlatform::Chatgpt => ("ChatGPT", "https://api.openai.com/v1", "gpt-4o-mini"),
        };
        PlatformPreset {
            platform: self,
            label: label.to_string(),
            default_base_url: base_url.to_string(),
            default_model: model.to_string(),
        }
    }
}

/// 平台预设信息（供设置页渲染）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformPreset {
    pub platform: AiPlatform,
    pub label: String,
    pub default_base_url: String,
    pub default_model: String,
}

/// 全部平台预设
pub fn all_presets() -> Vec<PlatformPreset> {
    [
        AiPlatform::Deepseek,
        AiPlatform::Qwen,
        AiPlatform::Glm,
        AiPlatform::Chatgpt,
    ]
    .into_iter()
    .map(AiPlatform::preset)
    .collect()
}

/// AI 配置（不含 API Key）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConfig {
    pub platform: AiPlatform,
    pub base_url: String,
    pub model: String,
    pub stream: bool,
    pub temperature: f32,
}

impl AiConfig {
    /// 以平台预设构造默认配置
    pub fn from_preset(platform: AiPlatform) -> Self {
        let preset = platform.preset();
        Self {
            platform,
            base_url: preset.default_base_url,
            model: preset.default_model,
            stream: true,
            temperature: 0.7,
        }
    }

    /// 基础校验：Base URL 与模型名不可为空，温度落在合理区间
    pub fn validate(&self) -> Result<(), String> {
        if self.base_url.trim().is_empty() {
            return Err("Base URL 不能为空".into());
        }
        if !self.base_url.starts_with("http://") && !self.base_url.starts_with("https://") {
            return Err("Base URL 需以 http:// 或 https:// 开头".into());
        }
        if self.model.trim().is_empty() {
            return Err("模型名不能为空".into());
        }
        if !(0.0..=2.0).contains(&self.temperature) {
            return Err("temperature 需在 0 ~ 2 之间".into());
        }
        Ok(())
    }
}

impl Default for AiConfig {
    fn default() -> Self {
        Self::from_preset(AiPlatform::Deepseek)
    }
}

/// 读取 settings 表中的原始值
pub fn load_setting(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    use rusqlite::OptionalExtension;
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |row| row.get::<_, String>(0),
    )
    .optional()
}

/// 写入 settings 表（UPSERT）
pub fn save_setting(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![key, value, crate::knowledge::store::now_ms()],
    )?;
    Ok(())
}

/// 读取 AI 配置；缺失或解析失败时回退到默认值
pub fn load_ai_config(conn: &Connection) -> rusqlite::Result<AiConfig> {
    let Some(raw) = load_setting(conn, SETTING_AI_CONFIG)? else {
        return Ok(AiConfig::default());
    };
    match serde_json::from_str::<AiConfig>(&raw) {
        Ok(config) => Ok(config),
        Err(err) => {
            eprintln!("AI 配置解析失败，回退默认值：{err}");
            Ok(AiConfig::default())
        }
    }
}

/// 保存 AI 配置
pub fn save_ai_config(conn: &Connection, config: &AiConfig) -> Result<(), String> {
    config.validate()?;
    let json = serde_json::to_string(config).map_err(|e| e.to_string())?;
    save_setting(conn, SETTING_AI_CONFIG, &json).map_err(|e| e.to_string())
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
    fn presets_cover_all_platforms_with_defaults() {
        let presets = all_presets();
        assert_eq!(presets.len(), 4);
        for preset in &presets {
            assert!(!preset.label.is_empty());
            assert!(preset.default_base_url.starts_with("https://"));
            assert!(!preset.default_model.is_empty());
        }
    }

    #[test]
    fn platform_parse_is_case_insensitive() {
        assert_eq!(AiPlatform::parse("DeepSeek"), Some(AiPlatform::Deepseek));
        assert_eq!(AiPlatform::parse("GLM"), Some(AiPlatform::Glm));
        assert_eq!(AiPlatform::parse("openai"), Some(AiPlatform::Chatgpt));
        assert_eq!(AiPlatform::parse("unknown"), None);
    }

    #[test]
    fn default_config_uses_deepseek_preset() {
        let config = AiConfig::default();
        assert_eq!(config.platform, AiPlatform::Deepseek);
        assert_eq!(config.base_url, "https://api.deepseek.com/v1");
        assert!(config.stream);
    }

    #[test]
    fn ai_config_round_trip() {
        with_db(|conn| {
            let config = AiConfig {
                platform: AiPlatform::Glm,
                base_url: "https://example.com/v1".into(),
                model: "glm-4-plus".into(),
                stream: false,
                temperature: 0.3,
            };
            save_ai_config(conn, &config).expect("save");
            let loaded = load_ai_config(conn).expect("load");
            assert_eq!(loaded, config);
        });
    }

    #[test]
    fn missing_config_falls_back_to_default() {
        with_db(|conn| {
            assert_eq!(load_ai_config(conn).expect("load"), AiConfig::default());
        });
    }

    #[test]
    fn corrupted_config_falls_back_to_default() {
        with_db(|conn| {
            save_setting(conn, SETTING_AI_CONFIG, "{ not json").expect("save raw");
            assert_eq!(load_ai_config(conn).expect("load"), AiConfig::default());
        });
    }

    #[test]
    fn validate_rejects_invalid_values() {
        let mut config = AiConfig::default();

        config.base_url = "".into();
        assert!(config.validate().is_err());

        config.base_url = "ftp://example.com".into();
        assert!(config.validate().is_err());

        config.base_url = "https://example.com".into();
        config.model = "  ".into();
        assert!(config.validate().is_err());

        config.model = "gpt".into();
        config.temperature = 3.0;
        assert!(config.validate().is_err());

        config.temperature = 0.5;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validate_accepts_temperature_boundaries() {
        let mut config = AiConfig::default();

        config.temperature = 0.0;
        assert!(config.validate().is_ok(), "温度下边界应合法");

        config.temperature = 2.0;
        assert!(config.validate().is_ok(), "温度上边界应合法");

        config.temperature = 2.0 + f32::EPSILON * 10.0;
        assert!(config.validate().is_err(), "超出上边界应拒绝");
    }

    #[test]
    fn save_ai_config_rejects_invalid() {
        with_db(|conn| {
            let mut config = AiConfig::default();
            config.model = "".into();
            assert!(save_ai_config(conn, &config).is_err());
        });
    }

    #[test]
    fn setting_upsert_overwrites_previous_value() {
        with_db(|conn| {
            save_setting(conn, "demo", "1").expect("first");
            save_setting(conn, "demo", "2").expect("second");
            assert_eq!(load_setting(conn, "demo").expect("load"), Some("2".into()));
        });
    }
}
