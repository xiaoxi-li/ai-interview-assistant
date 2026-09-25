use serde::Serialize;
use tauri::Manager;

mod commands;
mod state;

pub mod knowledge;
pub mod providers;
pub mod storage;

/// 应用基础信息（用于验证前后端 IPC 通路）
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
}

#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        name: "AI 面试助手".into(),
        version: env!("CARGO_PKG_VERSION").into(),
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // 数据库存放于系统应用数据目录
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("ai-interview.db");
            let db = storage::Db::open(&db_path)?;
            app.manage(state::AppState::new(db.into_shared_conn(), db_path));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            commands::list_notes,
            commands::list_groups,
            commands::import_markdown,
            commands::update_note,
            commands::create_note,
            commands::list_trash_notes,
            commands::delete_note,
            commands::restore_note,
            commands::purge_note,
            commands::empty_trash,
            commands::move_note,
            commands::create_group,
            commands::rename_group,
            commands::delete_group,
            commands::move_group,
            commands::reorder_groups,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_info_contains_name_and_version() {
        let info = app_info();
        assert_eq!(info.name, "AI 面试助手");
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn app_info_is_serializable() {
        let json = serde_json::to_string(&app_info()).expect("serialize app_info");
        assert!(json.contains("\"name\""));
        assert!(json.contains("\"version\""));
    }
}
