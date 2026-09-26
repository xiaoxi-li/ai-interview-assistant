use serde::Serialize;
use std::sync::Arc;
use tauri::Manager;

mod commands;
mod state;

pub mod audio;
pub mod config;
pub mod knowledge;
pub mod providers;
pub mod retrieval;
pub mod security;
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
            // P0 使用确定性 Mock 嵌入；真实本地 ONNX 模型在模型管理中台接入后替换
            let embedding: Arc<dyn providers::EmbeddingProvider> =
                Arc::new(providers::MockEmbeddingProvider::default());
            let secrets: Arc<dyn security::SecretStore> =
                Arc::new(security::KeyringStore::new(security::SERVICE_NAME));
            app.manage(state::AppState::new(
                db.into_shared_conn(),
                db_path,
                embedding,
                secrets,
            ));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            commands::list_notes,
            commands::list_groups,
            commands::import_markdown,
            commands::update_note,
            commands::search_notes,
            commands::search_similar_notes,
            commands::search_hybrid_notes,
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
            commands::list_tags,
            commands::create_tag,
            commands::rename_tag,
            commands::delete_tag,
            commands::set_note_tags,
            commands::list_note_versions,
            commands::restore_note_version,
            commands::save_api_key,
            commands::get_api_key_status,
            commands::delete_api_key,
            commands::list_platform_presets,
            commands::get_ai_config,
            commands::save_ai_config,
            commands::test_ai_connection,
            commands::list_asr_providers,
            commands::get_asr_config,
            commands::save_asr_config,
            commands::get_asr_credential_status,
            commands::save_asr_credential,
            commands::delete_asr_credentials,
            commands::list_audio_devices,
            commands::get_audio_config,
            commands::save_audio_config,
            commands::export_notes_markdown,
            commands::export_backup_json,
            commands::backup_database,
            commands::restore_backup,
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
