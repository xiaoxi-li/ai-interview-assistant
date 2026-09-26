//! 自动更新配置（P2）。

use super::{load_setting, save_setting};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// settings 表中更新配置的键名
pub const SETTING_UPDATE_CONFIG: &str = "app.update";

/// 更新检查配置
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateConfig {
    /// 启动时自动检查更新
    pub auto_check: bool,
    /// 更新元数据地址（Tauri updater 的 `latest.json` 或等价的 JSON）
    pub endpoint: String,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            auto_check: false,
            endpoint: String::new(),
        }
    }
}

impl UpdateConfig {
    pub fn validate(&self) -> Result<(), String> {
        let endpoint = self.endpoint.trim();
        if !endpoint.is_empty() && !endpoint.starts_with("https://") && !endpoint.starts_with("http://")
        {
            return Err("更新地址需以 http(s):// 开头".into());
        }
        Ok(())
    }

    /// 是否已配置更新源
    pub fn has_endpoint(&self) -> bool {
        !self.endpoint.trim().is_empty()
    }
}

/// 读取更新配置；缺失或解析失败时回退默认值
pub fn load_update_config(conn: &Connection) -> rusqlite::Result<UpdateConfig> {
    let Some(raw) = load_setting(conn, SETTING_UPDATE_CONFIG)? else {
        return Ok(UpdateConfig::default());
    };
    match serde_json::from_str::<UpdateConfig>(&raw) {
        Ok(config) if config.validate().is_ok() => Ok(config),
        Ok(_) => Ok(UpdateConfig::default()),
        Err(err) => {
            tracing::warn!("更新配置解析失败，回退默认值：{err}");
            Ok(UpdateConfig::default())
        }
    }
}

/// 保存更新配置
pub fn save_update_config(conn: &Connection, config: &UpdateConfig) -> Result<(), String> {
    config.validate()?;
    let json = serde_json::to_string(config).map_err(|e| e.to_string())?;
    save_setting(conn, SETTING_UPDATE_CONFIG, &json).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Db;

    #[test]
    fn default_is_disabled_without_endpoint() {
        let config = UpdateConfig::default();
        assert!(!config.auto_check);
        assert!(!config.has_endpoint());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn round_trip_persists_config() {
        let db = Db::open_in_memory().expect("db");
        let config = UpdateConfig {
            auto_check: true,
            endpoint: "https://example.com/latest.json".into(),
        };
        save_update_config(db.conn(), &config).expect("save");
        assert_eq!(load_update_config(db.conn()).expect("load"), config);
        assert!(load_update_config(db.conn()).expect("load").has_endpoint());
    }

    #[test]
    fn missing_config_falls_back_to_default() {
        let db = Db::open_in_memory().expect("db");
        assert_eq!(
            load_update_config(db.conn()).expect("load"),
            UpdateConfig::default()
        );
    }

    #[test]
    fn validate_rejects_invalid_endpoint() {
        let config = UpdateConfig {
            auto_check: true,
            endpoint: "example.com/latest.json".into(),
        };
        assert!(config.validate().is_err());

        let config = UpdateConfig {
            auto_check: true,
            endpoint: "https://example.com/latest.json".into(),
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn save_rejects_invalid_config() {
        let db = Db::open_in_memory().expect("db");
        let config = UpdateConfig {
            auto_check: false,
            endpoint: "ftp://example.com".into(),
        };
        assert!(save_update_config(db.conn(), &config).is_err());
    }
}
