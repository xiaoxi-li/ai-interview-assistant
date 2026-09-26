//! 全局快捷键（IA-11）：把「界面设置」中的按键绑定注册为系统级热键。
//!
//! 热键触发后不在此处执行业务逻辑，而是广播 `shortcut://action` 事件，
//! 由前端统一分发（与 IA-12 应用内快捷键共用同一套动作），避免逻辑重复。

use crate::config::interface::HotkeyBinding;
use tauri::Emitter;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
use std::sync::{Mutex, OnceLock};

/// 已注册快捷键与动作的映射（用于反查触发的动作）
fn registry() -> &'static Mutex<Vec<(Shortcut, String)>> {
    static REGISTRY: OnceLock<Mutex<Vec<(Shortcut, String)>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(Vec::new()))
}

/// 按配置重新注册全部全局快捷键；返回未能注册的项（含原因）
pub fn apply(app: &tauri::AppHandle, bindings: &[HotkeyBinding]) -> Vec<String> {
    let shortcuts = app.global_shortcut();
    let mut failed = Vec::new();

    if let Err(err) = shortcuts.unregister_all() {
        failed.push(format!("清理旧快捷键失败：{err}"));
    }

    let mut registered: Vec<(Shortcut, String)> = Vec::new();
    for binding in bindings {
        match binding.accelerator.parse::<Shortcut>() {
            Ok(shortcut) => match shortcuts.register(shortcut.clone()) {
                Ok(()) => registered.push((shortcut, binding.action.clone())),
                Err(err) => failed.push(format!(
                    "{}（{}）注册失败：{err}",
                    binding.action, binding.accelerator
                )),
            },
            Err(err) => failed.push(format!(
                "{}（{}）格式无效：{err}",
                binding.action, binding.accelerator
            )),
        }
    }

    match registry().lock() {
        Ok(mut guard) => *guard = registered,
        Err(err) => failed.push(format!("快捷键映射锁已中毒：{err}")),
    }

    failed
}

/// 热键触发：反查动作并广播给前端
pub fn dispatch(app: &tauri::AppHandle, shortcut: &Shortcut) {
    let action = match registry().lock() {
        Ok(guard) => guard
            .iter()
            .find(|(registered, _)| registered == shortcut)
            .map(|(_, action)| action.clone()),
        Err(err) => {
            tracing::warn!("快捷键映射锁已中毒：{err}");
            None
        }
    };

    if let Some(action) = action {
        let _ = app.emit(
            "shortcut://action",
            serde_json::json!({ "action": action, "source": "global" }),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_equality_enables_action_lookup() {
        // 反查动作依赖 Shortcut 的相等比较（真实注册需要 Tauri 运行时）
        let left = "Ctrl+Shift+A".parse::<Shortcut>().expect("parse left");
        let right = "Ctrl+Shift+A".parse::<Shortcut>().expect("parse right");
        let other = "Ctrl+Shift+B".parse::<Shortcut>().expect("parse other");

        assert_eq!(left, right, "相同加速键应可反查同一步");
        assert_ne!(left, other);
    }

    #[test]
    fn accelerator_parsing_supports_configured_defaults() {
        for accelerator in ["Ctrl+Shift+A", "Ctrl+Shift+R", "Ctrl+Shift+C", "Ctrl+Shift+P", "Ctrl+Shift+M"] {
            assert!(
                accelerator.parse::<Shortcut>().is_ok(),
                "默认快捷键应可解析：{accelerator}"
            );
        }
    }

    #[test]
    fn invalid_accelerator_is_rejected() {
        assert!("不是快捷键".parse::<Shortcut>().is_err());
    }
}
