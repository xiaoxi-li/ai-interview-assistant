use serde::Serialize;
use std::sync::Arc;
use tauri::Manager;

mod commands;
mod state;

pub mod asr;
pub mod audio;
pub mod config;
pub mod demo;
pub mod hotkeys;
pub mod interview;
pub mod knowledge;
pub mod logging;
pub mod models;
pub mod providers;
pub mod retrieval;
pub mod security;
pub mod storage;
pub mod tray;
pub mod update;

/// 应用基础信息（用于验证前后端 IPC 通路）
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
}

#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        name: "轻松面试".into(),
        version: env!("CARGO_PKG_VERSION").into(),
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    // 仅响应按下，避免一次按键触发两次
                    if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        hotkeys::dispatch(app, shortcut);
                    }
                })
                .build(),
        )
        .setup(|app| {
            // 数据库存放于系统应用数据目录
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;

            // 结构化日志（分级 + 脱敏 + 按天滚动）；初始化失败不阻断启动
            match logging::init_once(&data_dir) {
                Ok(dir) => tracing::info!(
                    log_dir = %dir.display(),
                    version = env!("CARGO_PKG_VERSION"),
                    "应用启动"
                ),
                // 此处日志系统尚未就绪，直接写标准错误
                Err(err) => eprintln!("日志初始化失败：{err}"),
            }

            let db_path = data_dir.join("easy-interview.db");
            let models_dir = data_dir.join("models");
            std::fs::create_dir_all(&models_dir)?;
            let db = storage::Db::open(&db_path)?;
            let did_rebuild_embedding = db.did_rebuild_embedding();
            // P0 使用确定性 Mock 嵌入；真实本地 ONNX 模型在模型管理中台接入后替换
            let embedding: Arc<dyn providers::EmbeddingProvider> =
                Arc::new(providers::MockEmbeddingProvider::default());
            let secrets: Arc<dyn security::SecretStore> =
                Arc::new(security::KeyringStore::new(security::SERVICE_NAME));
            let conn = db.into_shared_conn();

            // 嵌入维度迁移后触发全量向量重索引（尽力而为，失败仅告警不阻断启动）
            if did_rebuild_embedding {
                match conn.lock() {
                    Ok(guard) => {
                        if let Err(err) = retrieval::index::reindex_all(&guard, embedding.as_ref()) {
                            tracing::warn!("嵌入维度迁移后全量重索引失败：{err}");
                        }
                    }
                    Err(err) => tracing::warn!("嵌入维度迁移后获取连接失败：{err}"),
                }
            }

            app.manage(state::AppState::new(
                conn,
                db_path,
                models_dir,
                embedding,
                secrets,
            ));

            // IA-11：配置已保存的隐身模式开关（置顶 / 防录屏）
            let interface_config = {
                use tauri::Manager;
                let app_state = app.state::<state::AppState>();
                let loaded = match app_state.lock() {
                    Ok(conn) => config::interface::load_interface_config(&conn).ok(),
                    Err(_) => None,
                };
                loaded.unwrap_or_default()
            };
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_always_on_top(interface_config.always_on_top);
                let _ = window.set_content_protected(interface_config.hide_from_capture);
            }

            // IA-11：注册全局快捷键（失败仅告警，不阻断启动）
            for failure in hotkeys::apply(app.handle(), &interface_config.hotkeys) {
                tracing::warn!("全局快捷键注册提示：{failure}");
            }

            // IA-11：系统托盘
            if let Err(err) = tray::create(app) {
                tracing::warn!("系统托盘创建失败：{err}");
            }

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
            commands::test_cloud_asr_connection,
            commands::get_asr_credential_status,
            commands::save_asr_credential,
            commands::delete_asr_credentials,
            commands::analyze_vad_wav,
            commands::list_loopback_devices,
            commands::audio_capture_status,
            commands::start_audio_capture,
            commands::stop_audio_capture,
            commands::asr_session_status,
            commands::set_asr_paused,
            commands::enter_mini_mode,
            commands::exit_mini_mode,
            commands::list_asr_engines,
            commands::start_asr_session,
            commands::start_dual_asr_session,
            commands::stop_asr_session,
            commands::list_transcript_segments,
            commands::list_transcript_finals,
            commands::trigger_question,
            commands::list_questions,
            commands::latest_question,
            commands::get_question,
            commands::get_question_config,
            commands::save_question_config,
            commands::analyze_question_text,
            commands::recall_for_question,
            commands::generate_answer,
            commands::start_demo_session,
            commands::list_audio_devices,
            commands::get_audio_config,
            commands::save_audio_config,
            commands::get_privacy_config,
            commands::save_privacy_config,
            commands::clear_local_data,
            commands::get_interface_config,
            commands::save_interface_config,
            commands::get_autostart,
            commands::set_autostart,
            commands::get_storage_paths,
            commands::get_knowledge_config,
            commands::save_knowledge_config,
            commands::list_models,
            commands::download_model,
            commands::delete_model,
            commands::export_notes_markdown,
            commands::export_backup_json,
            commands::backup_database,
            commands::restore_backup,
            commands::set_stealth_window,
            commands::app_version,
            commands::get_update_config,
            commands::save_update_config,
            commands::check_app_update,
            commands::get_onboarding_state,
            commands::complete_onboarding,
            commands::list_sessions,
            commands::get_session_detail,
            commands::delete_session,
            commands::export_session,
            commands::apply_global_hotkeys,
        ])
        // 托盘行为：关闭主窗口时按配置最小化到托盘（IA-11）
        .on_window_event(|window, event| {
            use tauri::Manager;
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let handle = window.app_handle();
                let behavior = handle
                    .try_state::<state::AppState>()
                    .and_then(|state| state.lock().ok().map(|conn| config::interface::load_interface_config(&conn)))
                    .and_then(Result::ok)
                    .map(|config| config.tray_behavior)
                    .unwrap_or(config::interface::TrayBehavior::Minimize);

                if behavior == config::interface::TrayBehavior::Minimize {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_info_contains_name_and_version() {
        let info = app_info();
        assert_eq!(info.name, "轻松面试");
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn app_info_is_serializable() {
        let json = serde_json::to_string(&app_info()).expect("serialize app_info");
        assert!(json.contains("\"name\""));
        assert!(json.contains("\"version\""));
    }
}
