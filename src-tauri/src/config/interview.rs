//! 面试辅助（问题识别）配置。

use super::{load_setting, save_setting};
use crate::interview::question::{DEFAULT_MERGE_WINDOW_MS, DEFAULT_MIN_CHARS, DEFAULT_THRESHOLD};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// settings 表中问题识别配置的键名
pub const SETTING_QUESTION_CONFIG: &str = "interview.question";

/// 问题识别配置
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionConfig {
    /// 是否自动识别（转写命中即产出问题）；关闭后仅支持快捷键手动触发
    pub auto_detect: bool,
    /// 判定阈值（0~1）
    pub threshold: f32,
    /// 最短字符数
    pub min_chars: usize,
    /// 跨段聚合时间窗（毫秒）
    pub merge_window_ms: i64,
    /// 识别到问题后是否自动生成回答（IA-08）
    #[serde(default = "default_true")]
    pub auto_answer: bool,
}

/// 新增字段的默认值（兼容旧配置文件）
fn default_true() -> bool {
    true
}

impl Default for QuestionConfig {
    fn default() -> Self {
        Self {
            auto_detect: true,
            threshold: DEFAULT_THRESHOLD,
            min_chars: DEFAULT_MIN_CHARS,
            merge_window_ms: DEFAULT_MERGE_WINDOW_MS,
            auto_answer: true,
        }
    }
}

impl QuestionConfig {
    pub fn validate(&self) -> Result<(), String> {
        if !(0.0..=1.0).contains(&self.threshold) {
            return Err("判定阈值需在 0 ~ 1 之间".into());
        }
        if !(1..=64).contains(&self.min_chars) {
            return Err("最短字符数需在 1 ~ 64 之间".into());
        }
        if !(500..=20_000).contains(&self.merge_window_ms) {
            return Err("聚合时间窗需在 500 ~ 20000 毫秒之间".into());
        }
        Ok(())
    }
}

/// 读取问题识别配置；缺失或解析失败时回退默认值
pub fn load_question_config(conn: &Connection) -> rusqlite::Result<QuestionConfig> {
    let Some(raw) = load_setting(conn, SETTING_QUESTION_CONFIG)? else {
        return Ok(QuestionConfig::default());
    };
    match serde_json::from_str::<QuestionConfig>(&raw) {
        Ok(config) if config.validate().is_ok() => Ok(config),
        Ok(_) => Ok(QuestionConfig::default()),
        Err(err) => {
            eprintln!("问题识别配置解析失败，回退默认值：{err}");
            Ok(QuestionConfig::default())
        }
    }
}

/// 保存问题识别配置
pub fn save_question_config(conn: &Connection, config: &QuestionConfig) -> Result<(), String> {
    config.validate()?;
    let json = serde_json::to_string(config).map_err(|e| e.to_string())?;
    save_setting(conn, SETTING_QUESTION_CONFIG, &json).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Db;

    #[test]
    fn default_is_auto_with_sane_limits() {
        let config = QuestionConfig::default();
        assert!(config.auto_detect);
        assert_eq!(config.threshold, DEFAULT_THRESHOLD);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn round_trip_persists_config() {
        let db = Db::open_in_memory().expect("db");
        let config = QuestionConfig {
            auto_detect: false,
            threshold: 0.7,
            min_chars: 6,
            merge_window_ms: 5_000,
            auto_answer: false,
        };
        save_question_config(db.conn(), &config).expect("save");
        assert_eq!(load_question_config(db.conn()).expect("load"), config);
    }

    #[test]
    fn legacy_config_without_auto_answer_defaults_to_true() {
        let db = Db::open_in_memory().expect("db");
        // 旧版本写入的 JSON 不含 autoAnswer 字段
        save_setting(
            db.conn(),
            SETTING_QUESTION_CONFIG,
            r#"{"autoDetect":true,"threshold":0.5,"minChars":4,"mergeWindowMs":3000}"#,
        )
        .expect("save legacy");

        let config = load_question_config(db.conn()).expect("load");
        assert!(config.auto_answer, "旧配置应默认开启自动回答");
    }

    #[test]
    fn missing_or_corrupted_config_falls_back() {
        let db = Db::open_in_memory().expect("db");
        assert_eq!(
            load_question_config(db.conn()).expect("load"),
            QuestionConfig::default()
        );

        save_setting(db.conn(), SETTING_QUESTION_CONFIG, "{ broken").expect("save raw");
        assert_eq!(
            load_question_config(db.conn()).expect("load"),
            QuestionConfig::default()
        );

        // 合法 JSON 但越界：同样回退默认
        save_setting(db.conn(), SETTING_QUESTION_CONFIG, r#"{"autoDetect":true,"threshold":9.0,"minChars":4,"mergeWindowMs":3000}"#)
            .expect("save out of range");
        assert_eq!(
            load_question_config(db.conn()).expect("load"),
            QuestionConfig::default()
        );
    }

    #[test]
    fn validate_rejects_invalid_fields() {
        let mut config = QuestionConfig::default();

        config.threshold = 1.5;
        assert!(config.validate().is_err());

        config.threshold = 0.5;
        config.min_chars = 0;
        assert!(config.validate().is_err());

        config.min_chars = 4;
        config.merge_window_ms = 100;
        assert!(config.validate().is_err());

        config.merge_window_ms = 3_000;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn save_rejects_invalid_config() {
        let db = Db::open_in_memory().expect("db");
        let mut config = QuestionConfig::default();
        config.threshold = -0.2;
        assert!(save_question_config(db.conn(), &config).is_err());
    }
}
