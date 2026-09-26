//! 知识库配置：存储位置与嵌入模型选择。
//!
//! 说明：数据库与模型目录的切换需要重新建立连接，
//! 因此仅持久化偏好，标记 `restart_required` 由界面提示用户重启生效。

use super::{load_setting, save_setting};
use crate::models::{catalog, ModelKind};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// settings 表中知识库配置的键名
pub const SETTING_KNOWLEDGE_CONFIG: &str = "knowledge.config";

/// 默认嵌入模型 ID
pub const DEFAULT_EMBEDDING_MODEL: &str = "embedding-minilm-zh";

/// 知识库配置
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeConfig {
    /// 数据库文件路径（None 表示使用默认应用数据目录）
    pub db_path: Option<String>,
    /// 模型目录（None 表示使用默认应用数据目录）
    pub models_dir: Option<String>,
    /// 嵌入模型 ID
    pub embedding_model: String,
}

impl Default for KnowledgeConfig {
    fn default() -> Self {
        Self {
            db_path: None,
            models_dir: None,
            embedding_model: DEFAULT_EMBEDDING_MODEL.to_string(),
        }
    }
}

impl KnowledgeConfig {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(path) = &self.db_path {
            validate_path(path, "数据库路径")?;
        }
        if let Some(path) = &self.models_dir {
            validate_path(path, "模型目录")?;
        }
        if !embedding_available(&self.embedding_model) {
            return Err(format!("未知的嵌入模型：{}", self.embedding_model));
        }
        Ok(())
    }

    /// 是否与当前运行配置不一致（需要重启生效）
    pub fn requires_restart(&self, current: &Self) -> bool {
        self.db_path != current.db_path || self.models_dir != current.models_dir
    }
}

fn validate_path(raw: &str, label: &str) -> Result<(), String> {
    if raw.trim().is_empty() {
        return Err(format!("{label}不能为空白"));
    }
    let path = Path::new(raw);
    if !path.is_absolute() {
        return Err(format!("{label}需为绝对路径"));
    }
    Ok(())
}

/// 可选嵌入模型列表
pub fn available_embedding_models() -> Vec<(String, String)> {
    catalog()
        .into_iter()
        .filter(|spec| spec.kind == ModelKind::Embedding)
        .map(|spec| (spec.id, spec.name))
        .collect()
}

fn embedding_available(id: &str) -> bool {
    available_embedding_models()
        .iter()
        .any(|(candidate, _)| candidate == id)
}

/// 读取知识库配置；缺失或解析失败时回退默认值
pub fn load_knowledge_config(conn: &Connection) -> rusqlite::Result<KnowledgeConfig> {
    let Some(raw) = load_setting(conn, SETTING_KNOWLEDGE_CONFIG)? else {
        return Ok(KnowledgeConfig::default());
    };
    match serde_json::from_str::<KnowledgeConfig>(&raw) {
        Ok(config) => Ok(config),
        Err(err) => {
            tracing::warn!("知识库配置解析失败，回退默认值：{err}");
            Ok(KnowledgeConfig::default())
        }
    }
}

/// 保存知识库配置
pub fn save_knowledge_config(conn: &Connection, config: &KnowledgeConfig) -> Result<(), String> {
    config.validate()?;
    let json = serde_json::to_string(config).map_err(|e| e.to_string())?;
    save_setting(conn, SETTING_KNOWLEDGE_CONFIG, &json).map_err(|e| e.to_string())
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
    fn default_uses_app_data_dir_and_default_model() {
        let config = KnowledgeConfig::default();
        assert!(config.db_path.is_none());
        assert!(config.models_dir.is_none());
        assert_eq!(config.embedding_model, DEFAULT_EMBEDDING_MODEL);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn knowledge_config_round_trip() {
        with_db(|conn| {
            let config = KnowledgeConfig {
                db_path: Some("D:/data/ai-interview.db".into()),
                models_dir: Some("D:/data/models".into()),
                embedding_model: DEFAULT_EMBEDDING_MODEL.into(),
            };
            save_knowledge_config(conn, &config).expect("save");
            assert_eq!(load_knowledge_config(conn).expect("load"), config);
        });
    }

    #[test]
    fn missing_or_corrupted_config_falls_back() {
        with_db(|conn| {
            assert_eq!(
                load_knowledge_config(conn).expect("load"),
                KnowledgeConfig::default()
            );
            save_setting(conn, SETTING_KNOWLEDGE_CONFIG, "[]").expect("save raw");
            assert_eq!(
                load_knowledge_config(conn).expect("load"),
                KnowledgeConfig::default()
            );
        });
    }

    #[test]
    fn validate_rejects_relative_or_blank_paths() {
        let mut config = KnowledgeConfig::default();

        config.db_path = Some("relative/path.db".into());
        assert!(config.validate().is_err(), "相对路径应被拒绝");

        config.db_path = Some("   ".into());
        assert!(config.validate().is_err());

        config.db_path = Some("D:/data/ai.db".into());
        config.models_dir = Some("models".into());
        assert!(config.validate().is_err());

        config.models_dir = None;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validate_rejects_unknown_embedding_model() {
        let mut config = KnowledgeConfig::default();
        config.embedding_model = "not-a-model".into();
        assert!(config.validate().is_err());
    }

    #[test]
    fn restart_required_only_for_path_changes() {
        let current = KnowledgeConfig::default();

        let mut model_only = current.clone();
        model_only.embedding_model = DEFAULT_EMBEDDING_MODEL.into();
        assert!(!model_only.requires_restart(&current));

        let mut path_changed = current.clone();
        path_changed.db_path = Some("D:/data/ai.db".into());
        assert!(path_changed.requires_restart(&current));
    }

    #[test]
    fn available_embedding_models_is_not_empty() {
        let models = available_embedding_models();
        assert!(models.iter().any(|(id, _)| id == DEFAULT_EMBEDDING_MODEL));
    }
}
