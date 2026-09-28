//! 界面设置：主题、全局快捷键、开机自启与托盘行为。

use super::{load_setting, save_setting};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// settings 表中界面配置的键名
pub const SETTING_INTERFACE_CONFIG: &str = "interface.config";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Dark,
    Light,
    /// 跟随操作系统深浅色（ST-13）；实际生效主题由前端解析
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrayBehavior {
    /// 关闭窗口时最小化到托盘
    Minimize,
    /// 关闭窗口时退出应用
    Quit,
}

/// 支持的快捷键动作（与 IA-12 快捷操作对应）
pub const HOTKEY_ACTIONS: [&str; 5] = [
    "ask",
    "regenerate",
    "copy_answer",
    "pause_capture",
    "toggle_mini",
];

/// 单个快捷键绑定
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyBinding {
    pub action: String,
    /// 加速键字符串，如 `Ctrl+Shift+A`
    pub accelerator: String,
}

/// 默认快捷键
pub fn default_hotkeys() -> Vec<HotkeyBinding> {
    [
        ("ask", "Ctrl+Shift+A"),
        ("regenerate", "Ctrl+Shift+R"),
        ("copy_answer", "Ctrl+Shift+C"),
        ("pause_capture", "Ctrl+Shift+P"),
        ("toggle_mini", "Ctrl+Shift+M"),
    ]
    .into_iter()
    .map(|(action, accelerator)| HotkeyBinding {
        action: action.to_string(),
        accelerator: accelerator.to_string(),
    })
    .collect()
}

/// 界面配置
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceConfig {
    pub theme: Theme,
    pub hotkeys: Vec<HotkeyBinding>,
    pub auto_start: bool,
    pub tray_behavior: TrayBehavior,
    /// 隐身模式：窗口置顶（IA-11）
    #[serde(default)]
    pub always_on_top: bool,
    /// 隐身模式：屏幕共享时隐藏窗口内容（防录屏，IA-11）
    #[serde(default)]
    pub hide_from_capture: bool,
}

impl Default for InterfaceConfig {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            hotkeys: default_hotkeys(),
            auto_start: false,
            tray_behavior: TrayBehavior::Minimize,
            always_on_top: false,
            hide_from_capture: false,
        }
    }
}

impl InterfaceConfig {
    pub fn validate(&self) -> Result<(), String> {
        let mut seen: Vec<String> = Vec::new();
        for binding in &self.hotkeys {
            if !HOTKEY_ACTIONS.contains(&binding.action.as_str()) {
                return Err(format!("未知的快捷键动作：{}", binding.action));
            }
            if binding.accelerator.trim().is_empty() {
                return Err(format!("动作 {} 的快捷键不能为空", binding.action));
            }
            let normalized = binding.accelerator.trim().to_lowercase();
            if seen.contains(&normalized) {
                return Err(format!("快捷键冲突：{}", binding.accelerator));
            }
            if !normalized.contains('+') {
                return Err(format!(
                    "快捷键需包含修饰键（如 Ctrl+Shift+A）：{}",
                    binding.accelerator
                ));
            }
            seen.push(normalized);
        }
        Ok(())
    }

    /// 取某个动作的加速键
    pub fn accelerator_for(&self, action: &str) -> Option<&str> {
        self.hotkeys
            .iter()
            .find(|binding| binding.action == action)
            .map(|binding| binding.accelerator.as_str())
    }
}

/// 读取界面配置；缺失或解析失败时回退默认值
pub fn load_interface_config(conn: &Connection) -> rusqlite::Result<InterfaceConfig> {
    let Some(raw) = load_setting(conn, SETTING_INTERFACE_CONFIG)? else {
        return Ok(InterfaceConfig::default());
    };
    match serde_json::from_str::<InterfaceConfig>(&raw) {
        Ok(config) => Ok(config),
        Err(err) => {
            tracing::warn!("界面配置解析失败，回退默认值：{err}");
            Ok(InterfaceConfig::default())
        }
    }
}

/// 保存界面配置
pub fn save_interface_config(conn: &Connection, config: &InterfaceConfig) -> Result<(), String> {
    config.validate()?;
    let json = serde_json::to_string(config).map_err(|e| e.to_string())?;
    save_setting(conn, SETTING_INTERFACE_CONFIG, &json).map_err(|e| e.to_string())
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
    fn default_config_is_dark_with_default_hotkeys() {
        let config = InterfaceConfig::default();
        assert_eq!(config.theme, Theme::Dark);
        assert_eq!(config.tray_behavior, TrayBehavior::Minimize);
        assert!(!config.auto_start);
        assert_eq!(config.hotkeys.len(), HOTKEY_ACTIONS.len());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn default_hotkeys_are_unique_and_include_modifiers() {
        let hotkeys = default_hotkeys();
        let mut accelerators: Vec<String> = hotkeys
            .iter()
            .map(|h| h.accelerator.to_lowercase())
            .collect();
        let total = accelerators.len();
        accelerators.sort();
        accelerators.dedup();
        assert_eq!(accelerators.len(), total);
        assert!(hotkeys.iter().all(|h| h.accelerator.contains('+')));
    }

    #[test]
    fn theme_variants_serialize_lowercase() {
        // 前端以字符串交互，取值必须是 "dark" / "light" / "system"（ST-13）
        assert_eq!(serde_json::to_string(&Theme::Dark).unwrap(), "\"dark\"");
        assert_eq!(serde_json::to_string(&Theme::Light).unwrap(), "\"light\"");
        assert_eq!(serde_json::to_string(&Theme::System).unwrap(), "\"system\"");
        assert_eq!(
            serde_json::from_str::<Theme>("\"system\"").unwrap(),
            Theme::System
        );
    }

    #[test]
    fn interface_config_round_trip_with_system_theme() {
        with_db(|conn| {
            let config = InterfaceConfig {
                theme: Theme::System,
                hotkeys: default_hotkeys(),
                auto_start: false,
                tray_behavior: TrayBehavior::Minimize,
                always_on_top: false,
                hide_from_capture: false,
            };
            save_interface_config(conn, &config).expect("save");
            assert_eq!(load_interface_config(conn).expect("load").theme, Theme::System);
        });
    }

    #[test]
    fn unknown_theme_value_falls_back_to_default() {
        with_db(|conn| {
            // 未知主题取值会导致整份配置反序列化失败，按既有策略回退默认配置
            save_setting(
                conn,
                SETTING_INTERFACE_CONFIG,
                r#"{"theme":"sepia","hotkeys":[],"autoStart":false,"trayBehavior":"minimize"}"#,
            )
            .expect("save raw");
            assert_eq!(
                load_interface_config(conn).expect("load").theme,
                Theme::Dark,
                "解析失败时回退默认主题"
            );
        });
    }

    #[test]
    fn interface_config_round_trip() {
        with_db(|conn| {
            let config = InterfaceConfig {
                theme: Theme::Light,
                hotkeys: default_hotkeys(),
                auto_start: true,
                tray_behavior: TrayBehavior::Quit,
                always_on_top: true,
                hide_from_capture: true,
            };
            save_interface_config(conn, &config).expect("save");
            assert_eq!(load_interface_config(conn).expect("load"), config);
        });
    }

    #[test]
    fn legacy_config_without_stealth_fields_defaults_to_off() {
        with_db(|conn| {
            // 旧版本写入的 JSON 不含隐身模式字段
            save_setting(
                conn,
                SETTING_INTERFACE_CONFIG,
                r#"{"theme":"dark","hotkeys":[{"action":"ask","accelerator":"Ctrl+Shift+A"}],"autoStart":false,"trayBehavior":"minimize"}"#,
            )
            .expect("save legacy");

            let config = load_interface_config(conn).expect("load");
            assert!(!config.always_on_top);
            assert!(!config.hide_from_capture);
            assert_eq!(config.hotkeys.len(), 1);
        });
    }

    #[test]
    fn missing_or_corrupted_config_falls_back() {
        with_db(|conn| {
            assert_eq!(
                load_interface_config(conn).expect("load"),
                InterfaceConfig::default()
            );
            save_setting(conn, SETTING_INTERFACE_CONFIG, "{}").expect("save raw");
            // 缺字段的 JSON 无法反序列化，应回退默认值
            assert_eq!(
                load_interface_config(conn).expect("load"),
                InterfaceConfig::default()
            );
        });
    }

    #[test]
    fn validate_rejects_unknown_action_and_blank_accelerator() {
        let mut config = InterfaceConfig::default();

        config.hotkeys.push(HotkeyBinding {
            action: "unknown".into(),
            accelerator: "Ctrl+K".into(),
        });
        assert!(config.validate().is_err());

        config.hotkeys.pop();
        config.hotkeys[0].accelerator = "  ".into();
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_rejects_duplicates_and_missing_modifier() {
        let mut config = InterfaceConfig::default();

        config.hotkeys[1].accelerator = config.hotkeys[0].accelerator.clone();
        assert!(config.validate().is_err());

        config.hotkeys[1].accelerator = "A".into();
        assert!(config.validate().is_err(), "缺少修饰键应被拒绝");

        config.hotkeys[1].accelerator = "Ctrl+Shift+R".into();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn accelerator_lookup_works() {
        let config = InterfaceConfig::default();
        assert_eq!(config.accelerator_for("ask"), Some("Ctrl+Shift+A"));
        assert_eq!(config.accelerator_for("missing"), None);
    }

    #[test]
    fn save_rejects_invalid_config() {
        with_db(|conn| {
            let mut config = InterfaceConfig::default();
            config.hotkeys[0].accelerator = String::new();
            assert!(save_interface_config(conn, &config).is_err());
        });
    }
}
