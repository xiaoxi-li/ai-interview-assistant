//! Tauri 命令：前端调用入口。
//!
//! 约定：所有命令返回 `Result<T, String>`，错误直接以字符串形式透传给前端。

use crate::asr::{
    AsrControl, AsrPipeline, AsrSession, LocalAsrEngine, SherpaOnnxEngine, TranscriptRepo,
    DEFAULT_PARTIAL_INTERVAL_MS,
};
use crate::audio::capture::{pcm16_to_samples, rms_level};
use crate::audio::vad::VadConfig;
use crate::audio::{AudioCapture, FRAME_SAMPLES};
use crate::config::asr::CLOUD_ASR_IMPLEMENTED;
use crate::config::interview::QuestionConfig;
use crate::interview::question::{QuestionDetector, QuestionScore};
use crate::interview::store::{QaPairDto, QaRepo};
use crate::interview::DetectedQuestion;
use crate::knowledge::export::{ExportSummary, RestoreSummary};
use crate::knowledge::store::{now_ms, GroupsRepo, NotesRepo, TagsRepo, VersionsRepo};
use crate::knowledge::{GroupDto, ImportOutcome, NoteDto, NoteVersionDto, TagDto};
use crate::providers::types::{AsrMode, Speaker, TranscriptEvent};
use crate::providers::AiProvider;
use crate::retrieval::SearchHit;
use crate::state::AppState;
use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;
use tauri::State;

/// 列出全部笔记（不含回收站），按更新时间倒序
#[tauri::command]
pub fn list_notes(state: State<'_, AppState>) -> Result<Vec<NoteDto>, String> {
    let conn = state.lock()?;
    NotesRepo::new(&conn).list(false).map_err(|e| e.to_string())
}

/// 更新笔记标题与正文
#[tauri::command]
pub fn update_note(
    state: State<'_, AppState>,
    id: String,
    title: String,
    content: String,
) -> Result<NoteDto, String> {
    let conn = state.lock()?;
    // 编辑前留存历史版本，便于回滚
    if let Err(err) = VersionsRepo::new(&conn).snapshot(&id) {
        eprintln!("版本快照失败（note={id}）：{err}");
    }

    let note = NotesRepo::new(&conn)
        .update(&id, &title, &content)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("笔记不存在：{id}"))?;
    index_note_quietly(&conn, &state, &note);
    Ok(note)
}

/// 列出笔记的历史版本（新→旧）
#[tauri::command]
pub fn list_note_versions(
    state: State<'_, AppState>,
    id: String,
    limit: Option<usize>,
) -> Result<Vec<NoteVersionDto>, String> {
    let conn = state.lock()?;
    VersionsRepo::new(&conn)
        .list(&id, limit.unwrap_or(50))
        .map_err(|e| e.to_string())
}

/// 回滚笔记到指定历史版本
#[tauri::command]
pub fn restore_note_version(
    state: State<'_, AppState>,
    version_id: String,
) -> Result<NoteDto, String> {
    let conn = state.lock()?;
    let note = VersionsRepo::new(&conn)
        .restore(&version_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("版本不存在：{version_id}"))?;

    crate::retrieval::sync_note(&conn, &note.id, &note.title, &note.content, &note.tags)
        .map_err(|e| e.to_string())?;
    index_note_quietly(&conn, &state, &note);
    Ok(note)
}

/// 全文检索笔记（FTS5 + jieba 中文分词）
#[tauri::command]
pub fn search_notes(
    state: State<'_, AppState>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<SearchHit>, String> {
    let conn = state.lock()?;
    crate::retrieval::search_notes(&conn, &query, limit.unwrap_or(20))
        .map_err(|e| e.to_string())
}

/// 语义检索笔记（分块向量 + sqlite-vec 相似度）
#[tauri::command]
pub fn search_similar_notes(
    state: State<'_, AppState>,
    query: String,
    top_k: Option<usize>,
) -> Result<Vec<SearchHit>, String> {
    let conn = state.lock()?;
    crate::retrieval::index::search_notes(
        &conn,
        state.embedding.as_ref(),
        &query,
        top_k.unwrap_or(10),
    )
    .map_err(|e| e.to_string())
}

/// 混合检索：BM25 + 向量 RRF 融合
#[tauri::command]
pub fn search_hybrid_notes(
    state: State<'_, AppState>,
    query: String,
    top_k: Option<usize>,
) -> Result<Vec<SearchHit>, String> {
    let conn = state.lock()?;
    crate::retrieval::search_hybrid(&conn, state.embedding.as_ref(), &query, top_k.unwrap_or(10))
        .map_err(|e| e.to_string())
}

/// 向量索引采用"尽力而为"策略：失败不阻断笔记写入，仅记录告警。
fn index_note_quietly(conn: &rusqlite::Connection, state: &AppState, note: &NoteDto) {
    if let Err(err) = crate::retrieval::index::reindex_note(conn, state.embedding.as_ref(), note) {
        eprintln!("向量索引失败（note={}）：{err}", note.id);
    }
}

/// 新建空白笔记
#[tauri::command]
pub fn create_note(
    state: State<'_, AppState>,
    group_id: Option<String>,
) -> Result<NoteDto, String> {
    let conn = state.lock()?;
    let note = NotesRepo::new(&conn)
        .create_empty(group_id.as_deref())
        .map_err(|e| e.to_string())?;
    index_note_quietly(&conn, &state, &note);
    Ok(note)
}

/// 列出回收站中的笔记
#[tauri::command]
pub fn list_trash_notes(state: State<'_, AppState>) -> Result<Vec<NoteDto>, String> {
    let conn = state.lock()?;
    NotesRepo::new(&conn).list_trash().map_err(|e| e.to_string())
}

/// 移入回收站
#[tauri::command]
pub fn delete_note(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    let conn = state.lock()?;
    NotesRepo::new(&conn).trash(&id).map_err(|e| e.to_string())
}

/// 从回收站恢复
#[tauri::command]
pub fn restore_note(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    let conn = state.lock()?;
    NotesRepo::new(&conn).restore(&id).map_err(|e| e.to_string())
}

/// 彻底删除笔记
#[tauri::command]
pub fn purge_note(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    let conn = state.lock()?;
    NotesRepo::new(&conn).purge(&id).map_err(|e| e.to_string())
}

/// 清空回收站，返回删除条数
#[tauri::command]
pub fn empty_trash(state: State<'_, AppState>) -> Result<usize, String> {
    let conn = state.lock()?;
    NotesRepo::new(&conn).empty_trash().map_err(|e| e.to_string())
}

/// 移动笔记到群组（group_id 为 null 表示移出群组）
#[tauri::command]
pub fn move_note(
    state: State<'_, AppState>,
    id: String,
    group_id: Option<String>,
) -> Result<bool, String> {
    let conn = state.lock()?;
    NotesRepo::new(&conn)
        .move_to_group(&id, group_id.as_deref())
        .map_err(|e| e.to_string())
}

/// 导出全部笔记为 Markdown（按群组层级建目录）
#[tauri::command]
pub fn export_notes_markdown(
    state: State<'_, AppState>,
    dest: String,
) -> Result<ExportSummary, String> {
    let conn = state.lock()?;
    crate::knowledge::export::export_notes_to_dir(&conn, std::path::Path::new(&dest))
        .map_err(|e| e.to_string())
}

/// 导出 JSON 逻辑备份
#[tauri::command]
pub fn export_backup_json(
    state: State<'_, AppState>,
    dest: String,
) -> Result<ExportSummary, String> {
    let conn = state.lock()?;
    crate::knowledge::export::export_backup_json(&conn, std::path::Path::new(&dest))
        .map_err(|e| e.to_string())
}

/// 数据库文件级备份（一致副本）
#[tauri::command]
pub fn backup_database(
    state: State<'_, AppState>,
    dest: String,
) -> Result<ExportSummary, String> {
    let conn = state.lock()?;
    crate::knowledge::export::backup_database_file(&conn, std::path::Path::new(&dest))
        .map_err(|e| e.to_string())
}

/// 从 JSON 备份恢复，并重建检索索引
#[tauri::command]
pub fn restore_backup(
    state: State<'_, AppState>,
    source: String,
) -> Result<RestoreSummary, String> {
    let conn = state.lock()?;
    let summary = crate::knowledge::export::restore_backup_json(
        &conn,
        std::path::Path::new(&source),
    )
    .map_err(|e| e.to_string())?;

    // 恢复后重建全文索引并补齐向量索引
    if let Ok(notes) = NotesRepo::new(&conn).list(true) {
        for note in &notes {
            if let Err(err) =
                crate::retrieval::sync_note(&conn, &note.id, &note.title, &note.content, &note.tags)
            {
                eprintln!("索引重建失败（note={}）：{err}", note.id);
            }
            index_note_quietly(&conn, &state, note);
        }
    }
    Ok(summary)
}

/// 读取界面配置
#[tauri::command]
pub fn get_interface_config(
    state: State<'_, AppState>,
) -> Result<crate::config::interface::InterfaceConfig, String> {
    let conn = state.lock()?;
    crate::config::interface::load_interface_config(&conn).map_err(|e| e.to_string())
}

/// 保存界面配置
#[tauri::command]
pub fn save_interface_config(
    state: State<'_, AppState>,
    config: crate::config::interface::InterfaceConfig,
) -> Result<crate::config::interface::InterfaceConfig, String> {
    let conn = state.lock()?;
    crate::config::interface::save_interface_config(&conn, &config)?;
    crate::config::interface::load_interface_config(&conn).map_err(|e| e.to_string())
}

/// 查询开机自启状态
#[tauri::command]
pub fn get_autostart(app: tauri::AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

/// 设置开机自启
#[tauri::command]
pub fn set_autostart(app: tauri::AppHandle, enabled: bool) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(|e| e.to_string())?;
    } else {
        manager.disable().map_err(|e| e.to_string())?;
    }
    manager.is_enabled().map_err(|e| e.to_string())
}

/// 当前生效的存储路径（用于界面展示默认值）
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoragePaths {
    pub db_path: String,
    pub models_dir: String,
}

/// 读取当前存储路径
#[tauri::command]
pub fn get_storage_paths(state: State<'_, AppState>) -> StoragePaths {
    StoragePaths {
        db_path: state.db_path.display().to_string(),
        models_dir: state.models_dir.display().to_string(),
    }
}

/// 读取知识库配置
#[tauri::command]
pub fn get_knowledge_config(
    state: State<'_, AppState>,
) -> Result<crate::config::knowledge::KnowledgeConfig, String> {
    let conn = state.lock()?;
    crate::config::knowledge::load_knowledge_config(&conn).map_err(|e| e.to_string())
}

/// 保存知识库配置（路径变更需重启生效）
#[tauri::command]
pub fn save_knowledge_config(
    state: State<'_, AppState>,
    config: crate::config::knowledge::KnowledgeConfig,
) -> Result<crate::config::knowledge::KnowledgeConfig, String> {
    let conn = state.lock()?;
    crate::config::knowledge::save_knowledge_config(&conn, &config)?;
    crate::config::knowledge::load_knowledge_config(&conn).map_err(|e| e.to_string())
}

/// 列出模型清单与安装状态
#[tauri::command]
pub fn list_models(state: State<'_, AppState>) -> Result<Vec<crate::models::ModelStatus>, String> {
    Ok(crate::models::list_status(&state.models_dir))
}

/// 下载模型（断点续传 + SHA256 校验），通过 `model://progress` 事件上报进度
#[tauri::command]
pub async fn download_model(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<String, String> {
    use tauri::Emitter;

    let spec = crate::models::find_spec(&id)
        .ok_or_else(|| format!("未知模型：{id}"))?;
    let models_dir = state.models_dir.clone();
    let base = crate::models::mirror_base();
    let model_id = spec.id.clone();

    let joined = tauri::async_runtime::spawn_blocking(move || {
        crate::models::download_model(&models_dir, &spec, &base, &mut |downloaded, total| {
            let _ = app.emit(
                "model://progress",
                serde_json::json!({
                    "id": model_id,
                    "downloaded": downloaded,
                    "total": total,
                }),
            );
        })
    })
    .await
    .map_err(|e| format!("下载任务异常：{e}"))?;

    joined.map(|path| path.display().to_string())
}

/// 删除已安装模型
#[tauri::command]
pub fn delete_model(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    let spec = crate::models::find_spec(&id)
        .ok_or_else(|| format!("未知模型：{id}"))?;
    crate::models::delete_model(&state.models_dir, &spec)
}

/// 读取隐私配置
#[tauri::command]
pub fn get_privacy_config(
    state: State<'_, AppState>,
) -> Result<crate::config::privacy::PrivacyConfig, String> {
    let conn = state.lock()?;
    crate::config::privacy::load_privacy_config(&conn).map_err(|e| e.to_string())
}

/// 保存隐私配置（上传开关需与 ASR 模式一致）
#[tauri::command]
pub fn save_privacy_config(
    state: State<'_, AppState>,
    config: crate::config::privacy::PrivacyConfig,
) -> Result<crate::config::privacy::PrivacyConfig, String> {
    let conn = state.lock()?;
    let asr = crate::config::asr::load_asr_config(&conn).map_err(|e| e.to_string())?;
    let cloud_enabled = asr.mode == crate::providers::types::AsrMode::Cloud;
    config.validate(cloud_enabled)?;
    crate::config::privacy::save_privacy_config(&conn, &config)?;
    crate::config::privacy::load_privacy_config(&conn).map_err(|e| e.to_string())
}

/// 清除全部本地业务数据（保留设置与钥匙链凭证）
#[tauri::command]
pub fn clear_local_data(
    state: State<'_, AppState>,
) -> Result<crate::knowledge::ClearSummary, String> {
    let conn = state.lock()?;
    crate::knowledge::clear_all_data(&conn).map_err(|e| e.to_string())
}

/// 对本地 WAV 文件执行 VAD 分析（自检/诊断用，音频不出本机）
#[tauri::command]
pub fn analyze_vad_wav(
    path: String,
    threshold: Option<f32>,
) -> Result<crate::audio::VadReport, String> {
    let samples = crate::audio::wav::read_wav_as_16k_mono(std::path::Path::new(&path))?;
    Ok(crate::audio::analyze_vad_samples(
        &samples,
        threshold.unwrap_or(0.5),
    ))
}

/// 音频采集状态
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureStatus {
    pub running: bool,
    /// 当前采集来源：mic / system
    pub source: Option<String>,
    pub mic_device: String,
    pub system_device: String,
    /// 是否检测到系统音频回环设备
    pub loopback_available: bool,
}

fn capture_status(state: &AppState) -> Result<CaptureStatus, String> {
    let manager = state.capture_lock()?;
    Ok(CaptureStatus {
        running: manager.running.is_some(),
        source: manager.running.clone(),
        mic_device: manager.mic.device_name().to_string(),
        system_device: manager.system.device_name().to_string(),
        loopback_available: !crate::audio::loopback_candidates().is_empty(),
    })
}

/// 列出可用于采集系统音频的回环设备
#[tauri::command]
pub fn list_loopback_devices() -> Vec<crate::audio::DeviceInfo> {
    crate::audio::loopback_candidates()
}

/// 查询当前采集状态
#[tauri::command]
pub fn audio_capture_status(state: State<'_, AppState>) -> Result<CaptureStatus, String> {
    capture_status(&state)
}

/// 启动音频采集（source: mic / system），并推送音量电平事件 `audio://level`
#[tauri::command]
pub fn start_audio_capture(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    source: String,
    device_id: Option<String>,
) -> Result<CaptureStatus, String> {
    use tauri::Emitter;

    // 电平试听与 ASR 会话互斥：ASR 泵独占采集事件流
    if state.asr_lock()?.is_running() {
        return Err("本地 ASR 会话进行中，采集事件流已被转写占用；请先停止转写再单独试听电平。".into());
    }

    let (events, source_label) = {
        let mut manager = state.capture_lock()?;
        // 单路采集：切换来源时先停止现有采集
        manager.stop_all();

        let capture = match source.as_str() {
            "mic" => &mut manager.mic,
            "system" => &mut manager.system,
            other => return Err(format!("未知的采集来源：{other}")),
        };

        let device = device_id.unwrap_or_default();
        capture.start(&device)?;
        let events = capture.take_events()?;
        manager.running = Some(source.clone());
        (events, source)
    };

    // 电平泵：把 PCM 帧换算为 RMS 并推送给前端（音频数据本身不出主进程）
    let app_handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        for chunk in events.iter() {
            if chunk.ts < 0 {
                let _ = app_handle.emit(
                    "audio://error",
                    serde_json::json!({ "source": source_label, "message": "音频流发生错误" }),
                );
                continue;
            }
            let samples = crate::audio::capture::pcm16_to_samples(&chunk.data);
            let level = crate::audio::capture::rms_level(&samples);
            let _ = app_handle.emit(
                "audio://level",
                serde_json::json!({
                    "source": source_label,
                    "level": level,
                    "ts": chunk.ts,
                    "frames": samples.len(),
                }),
            );
        }
    });

    capture_status(&state)
}

/// 停止音频采集
#[tauri::command]
pub fn stop_audio_capture(state: State<'_, AppState>) -> Result<CaptureStatus, String> {
    {
        let mut manager = state.capture_lock()?;
        manager.stop_all();
    }
    capture_status(&state)
}

// ==================== 本地 ASR（P0 唯一通路） ====================

/// 本地 ASR 会话状态
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrSessionStatus {
    pub running: bool,
    pub session_id: Option<String>,
    pub source: Option<String>,
    /// `me` / `interviewer`
    pub speaker: Option<String>,
    pub engine: Option<String>,
    pub started_at: Option<i64>,
}

fn asr_status(state: &AppState) -> Result<AsrSessionStatus, String> {
    let runtime = state.asr_lock()?;
    Ok(match &runtime.session {
        Some(session) => AsrSessionStatus {
            running: true,
            session_id: Some(session.id.clone()),
            source: Some(session.source.clone()),
            speaker: Some(session.speaker.as_str().to_string()),
            engine: Some(session.engine.clone()),
            started_at: Some(session.started_at),
        },
        None => AsrSessionStatus {
            running: false,
            session_id: None,
            source: None,
            speaker: None,
            engine: None,
            started_at: None,
        },
    })
}

/// 查询本地 ASR 会话状态
#[tauri::command]
pub fn asr_session_status(state: State<'_, AppState>) -> Result<AsrSessionStatus, String> {
    asr_status(&state)
}

/// 列出本地可用的 ASR 引擎及其就绪状态
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrEngineInfo {
    pub id: String,
    pub label: String,
    /// 是否可直接使用（模型与推理均就绪）
    pub ready: bool,
    pub detail: String,
}

#[tauri::command]
pub fn list_asr_engines(state: State<'_, AppState>) -> Vec<AsrEngineInfo> {
    let language = state
        .lock()
        .ok()
        .and_then(|conn| crate::config::asr::load_asr_config(&conn).ok())
        .map(|config| config.language)
        .unwrap_or_else(|| "zh-CN".to_string());

    let (ready, detail) = match SherpaOnnxEngine::new(&state.models_dir, &language) {
        Ok(engine) => (
            false,
            format!(
                "模型已就绪（{}）；native 推理尚未接入",
                engine.model_path().display()
            ),
        ),
        Err(err) => (false, err.to_string()),
    };

    vec![AsrEngineInfo {
        id: crate::asr::LOCAL_ASR_MODEL_ID.to_string(),
        label: "本地 ASR（sherpa-onnx 中文 paraformer）".to_string(),
        ready,
        detail,
    }]
}

/// 广播转写事件并按需落库
fn handle_transcript_events(
    app: &tauri::AppHandle,
    conn: &std::sync::Arc<std::sync::Mutex<rusqlite::Connection>>,
    session_id: &str,
    events: &[TranscriptEvent],
) {
    use tauri::Emitter;

    for event in events {
        let _ = app.emit(
            "asr://transcript",
            serde_json::json!({
                "sessionId": session_id,
                "event": event,
                "ts": now_ms(),
            }),
        );

        // 仅 final 落库：partial 会被后续结果覆盖，落库意义有限
        if let TranscriptEvent::Final { speaker, text } = event {
            match conn.lock() {
                Ok(guard) => {
                    if let Err(err) =
                        TranscriptRepo::new(&guard).append(session_id, *speaker, text, true, now_ms())
                    {
                        eprintln!("转写落库失败（session={session_id}）：{err}");
                    }
                }
                Err(err) => eprintln!("转写落库跳过（连接锁中毒）：{err}"),
            }
        }
    }
}

/// 启动本地 ASR 会话（默认走本地引擎；采样→VAD 分段→识别→事件+落库）
#[tauri::command]
pub fn start_asr_session(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    source: String,
    device_id: Option<String>,
) -> Result<AsrSessionStatus, String> {
    let speaker = crate::asr::speaker_for_source(&source)
        .ok_or_else(|| format!("未知采集来源：{source}"))?;

    // 1) 读取配置并强制本地通路
    let config = {
        let conn = state.lock()?;
        crate::config::asr::load_asr_config(&conn).map_err(|e| e.to_string())?
    };
    if config.mode == AsrMode::Cloud && !CLOUD_ASR_IMPLEMENTED {
        return Err(
            "云端 ASR 尚未接入（计划于 P1）。请到「设置 → 语音识别」将模式切换为「本地（隐私优先）」后再开始转写。"
                .into(),
        );
    }

    // 问题识别配置（IA-06）
    let question_config = {
        let conn = state.lock()?;
        crate::config::interview::load_question_config(&conn).map_err(|e| e.to_string())?
    };

    // 2) 构造本地引擎；模型缺失时返回可操作错误
    let engine = SherpaOnnxEngine::new(&state.models_dir, &config.language)
        .map_err(|e| e.to_string())?;
    let engine_name = engine.name().to_string();

    // 3) 停止既有会话与采集，启动新采集
    {
        let mut runtime = state.asr_lock()?;
        runtime.stop();
    }
    let audio_rx = {
        let mut manager = state.capture_lock()?;
        manager.stop_all();
        let capture = match source.as_str() {
            "mic" => &mut manager.mic,
            "system" => &mut manager.system,
            other => return Err(format!("未知的采集来源：{other}")),
        };
        capture.start(device_id.as_deref().unwrap_or(""))?;
        let audio_rx = capture.take_events()?;
        manager.running = Some(source.clone());
        audio_rx
    };

    // 4) 建会话记录 + 启动采集泵
    let session = {
        let conn = state.lock()?;
        TranscriptRepo::new(&conn)
            .start_session(&format!("面试会话 · {source}"))
            .map_err(|e| e.to_string())?
    };

    let (control_tx, control_rx) = mpsc::channel::<AsrControl>();
    let app_handle = app.clone();
    let conn_handle = state.conn.clone();
    let session_id = session.id.clone();
    let threshold = config.vad_threshold;
    let auto_detect = question_config.auto_detect;
    let question_threshold = question_config.threshold;
    let question_min_chars = question_config.min_chars;
    let question_window_ms = question_config.merge_window_ms;

    let pump = thread::spawn(move || {
        let mut pipeline = AsrPipeline::new(
            Box::new(engine),
            speaker,
            VadConfig::default(),
            threshold,
            DEFAULT_PARTIAL_INTERVAL_MS,
        );
        let mut detector =
            QuestionDetector::new(question_threshold, question_min_chars, question_window_ms);

        let mut stopping = false;
        loop {
            // 处理控制指令（停止 / 手动触发问题识别）
            while let Ok(control) = control_rx.try_recv() {
                match control {
                    AsrControl::Stop => stopping = true,
                    AsrControl::TriggerQuestion => {
                        if let Some(question) = detector.trigger(now_ms()) {
                            handle_detected_question(
                                &app_handle,
                                &conn_handle,
                                &session_id,
                                &question,
                                true,
                            );
                        }
                    }
                }
            }
            if stopping {
                break;
            }

            match audio_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(chunk) => {
                    if chunk.ts < 0 {
                        handle_transcript_events(
                            &app_handle,
                            &conn_handle,
                            &session_id,
                            &[TranscriptEvent::Error {
                                message: "音频流发生错误，请检查设备后重试".into(),
                            }],
                        );
                        continue;
                    }

                    let samples = pcm16_to_samples(&chunk.data);
                    let level = rms_level(&samples);
                    {
                        use tauri::Emitter;
                        let _ = app_handle.emit(
                            "audio://level",
                            serde_json::json!({ "source": "asr", "level": level, "ts": chunk.ts }),
                        );
                    }

                    for frame in samples.chunks(FRAME_SAMPLES) {
                        if frame.len() < FRAME_SAMPLES {
                            break;
                        }
                        let frame_events = pipeline.push_frame(frame, chunk.ts);
                        if !frame_events.is_empty() {
                            handle_transcript_events(
                                &app_handle,
                                &conn_handle,
                                &session_id,
                                &frame_events,
                            );
                            feed_questions(
                                &mut detector,
                                &app_handle,
                                &conn_handle,
                                &session_id,
                                auto_detect,
                                &frame_events,
                            );
                        }
                    }
                }
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }

        // 停止/断流：冲刷未完成的语音段与问题缓冲
        let tail = pipeline.flush();
        if !tail.is_empty() {
            handle_transcript_events(&app_handle, &conn_handle, &session_id, &tail);
            feed_questions(
                &mut detector,
                &app_handle,
                &conn_handle,
                &session_id,
                auto_detect,
                &tail,
            );
        }
        if auto_detect {
            if let Some(question) = detector.flush() {
                handle_detected_question(&app_handle, &conn_handle, &session_id, &question, false);
            }
        }
    });

    {
        let mut runtime = state.asr_lock()?;
        runtime.session = Some(AsrSession::new(
            session.id,
            source,
            speaker,
            engine_name,
            session.started_at,
            control_tx,
            pump,
        ));
    }

    asr_status(&state)
}

/// 停止本地 ASR 会话
#[tauri::command]
pub fn stop_asr_session(state: State<'_, AppState>) -> Result<AsrSessionStatus, String> {
    let session_id = {
        let mut runtime = state.asr_lock()?;
        let id = runtime.session_id();
        runtime.stop();
        id
    };

    {
        let mut manager = state.capture_lock()?;
        manager.stop_all();
    }

    if let Some(id) = session_id {
        let conn = state.lock()?;
        if let Err(err) = TranscriptRepo::new(&conn).end_session(&id) {
            eprintln!("结束会话失败（session={id}）：{err}");
        }
    }

    asr_status(&state)
}

/// 读取某会话的转写段落（含 partial 记录，按时间正序）
#[tauri::command]
pub fn list_transcript_segments(
    state: State<'_, AppState>,
    session_id: String,
    limit: Option<usize>,
) -> Result<Vec<crate::asr::TranscriptSegmentDto>, String> {
    let conn = state.lock()?;
    TranscriptRepo::new(&conn)
        .list_segments(&session_id, limit.unwrap_or(500))
        .map_err(|e| e.to_string())
}

/// 读取某会话的最终转写文本
#[tauri::command]
pub fn list_transcript_finals(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<Vec<crate::asr::TranscriptSegmentDto>, String> {
    let conn = state.lock()?;
    TranscriptRepo::new(&conn)
        .list_finals(&session_id)
        .map_err(|e| e.to_string())
}

// ==================== 问题识别（IA-06） ====================

/// 记录识别出的问题并广播（`interview://question`）
fn handle_detected_question(
    app: &tauri::AppHandle,
    conn: &std::sync::Arc<std::sync::Mutex<rusqlite::Connection>>,
    session_id: &str,
    question: &DetectedQuestion,
    manual: bool,
) {
    use tauri::Emitter;

    let qa_id = match conn.lock() {
        Ok(guard) => QaRepo::new(&guard)
            .create_question(session_id, &question.text, question.ts)
            .map(|pair| pair.id)
            .map_err(|err| eprintln!("问题落库失败（session={session_id}）：{err}"))
            .ok(),
        Err(err) => {
            eprintln!("问题落库跳过（连接锁中毒）：{err}");
            None
        }
    };

    let _ = app.emit(
        "interview://question",
        serde_json::json!({
            "sessionId": session_id,
            "manual": manual,
            "question": question,
            "qaId": qa_id,
        }),
    );
}

/// 从转写事件中筛出面试官发言并送入问题检测器（自动模式下命中即产出）
fn feed_questions(
    detector: &mut QuestionDetector,
    app: &tauri::AppHandle,
    conn: &std::sync::Arc<std::sync::Mutex<rusqlite::Connection>>,
    session_id: &str,
    auto_detect: bool,
    events: &[TranscriptEvent],
) {
    for event in events {
        if let TranscriptEvent::Final {
            speaker: Speaker::Interviewer,
            text,
        } = event
        {
            if let Some(question) = detector.push(text, now_ms()) {
                if auto_detect {
                    handle_detected_question(app, conn, session_id, &question, false);
                }
            }
        }
    }
}

/// 手动触发一次问题识别（供快捷键 / 界面按钮调用）
#[tauri::command]
pub fn trigger_question(state: State<'_, AppState>) -> Result<(), String> {
    let runtime = state.asr_lock()?;
    let session = runtime
        .session
        .as_ref()
        .ok_or_else(|| "当前没有进行中的转写会话，无法识别问题".to_string())?;
    session.send(AsrControl::TriggerQuestion)
}

/// 读取某会话已识别的问题（按时间正序）
#[tauri::command]
pub fn list_questions(
    state: State<'_, AppState>,
    session_id: String,
    limit: Option<usize>,
) -> Result<Vec<QaPairDto>, String> {
    let conn = state.lock()?;
    QaRepo::new(&conn)
        .list(&session_id, limit.unwrap_or(100))
        .map_err(|e| e.to_string())
}

/// 读取最近识别出的问题
#[tauri::command]
pub fn latest_question(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<Option<QaPairDto>, String> {
    let conn = state.lock()?;
    QaRepo::new(&conn)
        .latest(&session_id)
        .map_err(|e| e.to_string())
}

/// 读取问题识别配置
#[tauri::command]
pub fn get_question_config(state: State<'_, AppState>) -> Result<QuestionConfig, String> {
    let conn = state.lock()?;
    crate::config::interview::load_question_config(&conn).map_err(|e| e.to_string())
}

/// 保存问题识别配置
#[tauri::command]
pub fn save_question_config(
    state: State<'_, AppState>,
    config: QuestionConfig,
) -> Result<QuestionConfig, String> {
    let conn = state.lock()?;
    crate::config::interview::save_question_config(&conn, &config)?;
    Ok(config)
}

/// 对给定文本做一次问题判定（界面预览 / 调试用）
#[tauri::command]
pub fn analyze_question_text(text: String) -> QuestionScore {
    crate::interview::question::analyze(&text)
}

/// 枚举音频设备（输入=麦克风，输出=系统音频来源）
#[tauri::command]
pub fn list_audio_devices() -> Vec<crate::audio::DeviceInfo> {
    crate::audio::list_devices()
}

/// 读取音频设备配置
#[tauri::command]
pub fn get_audio_config(
    state: State<'_, AppState>,
) -> Result<crate::config::audio::AudioConfig, String> {
    let conn = state.lock()?;
    crate::config::audio::load_audio_config(&conn).map_err(|e| e.to_string())
}

/// 保存音频设备配置
#[tauri::command]
pub fn save_audio_config(
    state: State<'_, AppState>,
    config: crate::config::audio::AudioConfig,
) -> Result<crate::config::audio::AudioConfig, String> {
    let conn = state.lock()?;
    crate::config::audio::save_audio_config(&conn, &config)?;
    crate::config::audio::load_audio_config(&conn).map_err(|e| e.to_string())
}

/// ASR 服务商预设（供设置页渲染）
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrProviderPreset {
    pub provider: String,
    pub label: String,
    pub default_model: String,
    pub credential_fields: Vec<String>,
}

/// 单个凭证字段的状态（不回传明文）
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialFieldStatus {
    pub field: String,
    pub configured: bool,
    pub masked: String,
}

/// 服务商凭证整体状态
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrCredentialStatus {
    pub provider: String,
    /// 是否所有必需字段均已配置
    pub complete: bool,
    pub fields: Vec<CredentialFieldStatus>,
}

fn credential_status(
    secrets: &dyn crate::security::SecretStore,
    provider: crate::config::asr::CloudAsrProvider,
) -> Result<AsrCredentialStatus, String> {
    let mut fields = Vec::new();
    let mut complete = true;
    for field in provider.credential_fields() {
        let reference = crate::security::asr_credential_ref(provider.as_str(), field);
        let masked = secrets.masked(&reference)?;
        if masked.is_none() {
            complete = false;
        }
        fields.push(CredentialFieldStatus {
            field: field.to_string(),
            configured: masked.is_some(),
            masked: masked.unwrap_or_default(),
        });
    }
    Ok(AsrCredentialStatus {
        provider: provider.as_str().to_string(),
        complete,
        fields,
    })
}

fn parse_provider(
    provider: &str,
) -> Result<crate::config::asr::CloudAsrProvider, String> {
    crate::config::asr::CloudAsrProvider::parse(provider)
        .ok_or_else(|| format!("未知的 ASR 服务商：{provider}"))
}

/// 列出全部云端 ASR 服务商预设
#[tauri::command]
pub fn list_asr_providers() -> Vec<AsrProviderPreset> {
    crate::config::asr::all_cloud_providers()
        .into_iter()
        .map(|provider| AsrProviderPreset {
            provider: provider.as_str().to_string(),
            label: provider.label().to_string(),
            default_model: provider.default_model().to_string(),
            credential_fields: provider
                .credential_fields()
                .iter()
                .map(|f| f.to_string())
                .collect(),
        })
        .collect()
}

/// 读取 ASR 配置
#[tauri::command]
pub fn get_asr_config(state: State<'_, AppState>) -> Result<crate::config::asr::AsrConfig, String> {
    let conn = state.lock()?;
    crate::config::asr::load_asr_config(&conn).map_err(|e| e.to_string())
}

/// 保存 ASR 配置
#[tauri::command]
pub fn save_asr_config(
    state: State<'_, AppState>,
    config: crate::config::asr::AsrConfig,
) -> Result<crate::config::asr::AsrConfig, String> {
    let conn = state.lock()?;
    crate::config::asr::save_asr_config(&conn, &config)?;
    crate::config::asr::load_asr_config(&conn).map_err(|e| e.to_string())
}

/// 查询服务商凭证配置状态
#[tauri::command]
pub fn get_asr_credential_status(
    state: State<'_, AppState>,
    provider: String,
) -> Result<AsrCredentialStatus, String> {
    let provider = parse_provider(&provider)?;
    credential_status(state.secrets.as_ref(), provider)
}

/// 保存单个凭证字段（空值表示删除该字段）
#[tauri::command]
pub fn save_asr_credential(
    state: State<'_, AppState>,
    provider: String,
    field: String,
    value: String,
) -> Result<AsrCredentialStatus, String> {
    let provider = parse_provider(&provider)?;
    if !provider.credential_fields().contains(&field.as_str()) {
        return Err(format!("服务商 {} 不支持凭证字段 {field}", provider.as_str()));
    }
    let reference = crate::security::asr_credential_ref(provider.as_str(), &field);
    let trimmed = value.trim();
    if trimmed.is_empty() {
        state.secrets.delete(&reference)?;
    } else {
        state.secrets.set(&reference, trimmed)?;
    }
    credential_status(state.secrets.as_ref(), provider)
}

/// 清空服务商全部凭证
#[tauri::command]
pub fn delete_asr_credentials(
    state: State<'_, AppState>,
    provider: String,
) -> Result<AsrCredentialStatus, String> {
    let provider = parse_provider(&provider)?;
    for field in provider.credential_fields() {
        state
            .secrets
            .delete(&crate::security::asr_credential_ref(provider.as_str(), field))?;
    }
    credential_status(state.secrets.as_ref(), provider)
}

/// 一键测试 AI 服务连通性（读取已保存配置与钥匙链中的 API Key）
#[tauri::command]
pub fn test_ai_connection(
    state: State<'_, AppState>,
) -> Result<crate::providers::types::ConnectionTest, String> {
    let config = {
        let conn = state.lock()?;
        crate::config::load_ai_config(&conn).map_err(|e| e.to_string())?
    };
    // 网络请求不持有数据库锁
    let api_key = state
        .secrets
        .get(&crate::security::api_key_ref(config.platform.as_str()))?;

    let provider = crate::providers::OpenAiCompatProvider::new(config, api_key);
    provider.test_connection().map_err(|e| e.to_string())
}

/// 列出全部 AI 平台预设
#[tauri::command]
pub fn list_platform_presets() -> Vec<crate::config::PlatformPreset> {
    crate::config::all_presets()
}

/// 读取当前 AI 配置
#[tauri::command]
pub fn get_ai_config(state: State<'_, AppState>) -> Result<crate::config::AiConfig, String> {
    let conn = state.lock()?;
    crate::config::load_ai_config(&conn).map_err(|e| e.to_string())
}

/// 保存 AI 配置
#[tauri::command]
pub fn save_ai_config(
    state: State<'_, AppState>,
    config: crate::config::AiConfig,
) -> Result<crate::config::AiConfig, String> {
    let conn = state.lock()?;
    crate::config::save_ai_config(&conn, &config)?;
    crate::config::load_ai_config(&conn).map_err(|e| e.to_string())
}

/// API Key 配置状态（不回传完整密钥）
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyStatus {
    pub platform: String,
    pub configured: bool,
    /// 掩码后的密钥，未配置时为空串
    pub masked: String,
}

fn api_key_status(secrets: &dyn crate::security::SecretStore, platform: &str) -> Result<ApiKeyStatus, String> {
    let reference = crate::security::api_key_ref(platform);
    let masked = secrets.masked(&reference)?;
    Ok(ApiKeyStatus {
        platform: platform.to_string(),
        configured: masked.is_some(),
        masked: masked.unwrap_or_default(),
    })
}

/// 保存 API Key 到系统钥匙链（传空串表示删除）
#[tauri::command]
pub fn save_api_key(
    state: State<'_, AppState>,
    platform: String,
    api_key: String,
) -> Result<ApiKeyStatus, String> {
    let reference = crate::security::api_key_ref(&platform);
    let trimmed = api_key.trim();
    if trimmed.is_empty() {
        state.secrets.delete(&reference)?;
    } else {
        state.secrets.set(&reference, trimmed)?;
    }
    api_key_status(state.secrets.as_ref(), &platform)
}

/// 查询 API Key 配置状态
#[tauri::command]
pub fn get_api_key_status(
    state: State<'_, AppState>,
    platform: String,
) -> Result<ApiKeyStatus, String> {
    api_key_status(state.secrets.as_ref(), &platform)
}

/// 删除 API Key
#[tauri::command]
pub fn delete_api_key(
    state: State<'_, AppState>,
    platform: String,
) -> Result<ApiKeyStatus, String> {
    state.secrets.delete(&crate::security::api_key_ref(&platform))?;
    api_key_status(state.secrets.as_ref(), &platform)
}

/// 列出全部标签
#[tauri::command]
pub fn list_tags(state: State<'_, AppState>) -> Result<Vec<TagDto>, String> {
    let conn = state.lock()?;
    TagsRepo::new(&conn).list().map_err(|e| e.to_string())
}

/// 创建标签（同名则复用）
#[tauri::command]
pub fn create_tag(state: State<'_, AppState>, name: String) -> Result<TagDto, String> {
    let conn = state.lock()?;
    TagsRepo::new(&conn).ensure(&name).map_err(|e| e.to_string())
}

/// 重命名标签
#[tauri::command]
pub fn rename_tag(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> Result<bool, String> {
    let conn = state.lock()?;
    TagsRepo::new(&conn)
        .rename(&id, &name)
        .map_err(|e| e.to_string())
}

/// 删除标签；受影响笔记的全文索引同步刷新
#[tauri::command]
pub fn delete_tag(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    let conn = state.lock()?;
    let repo = TagsRepo::new(&conn);
    let affected = repo.note_ids_by_tag_id(&id).map_err(|e| e.to_string())?;

    let deleted = repo.delete(&id).map_err(|e| e.to_string())?;
    if deleted {
        resync_notes(&conn, &affected);
    }
    Ok(deleted)
}

/// 覆盖设置笔记标签，并刷新检索索引
#[tauri::command]
pub fn set_note_tags(
    state: State<'_, AppState>,
    id: String,
    tags: Vec<String>,
) -> Result<NoteDto, String> {
    let conn = state.lock()?;
    TagsRepo::new(&conn)
        .set_note_tags(&id, &tags)
        .map_err(|e| e.to_string())?;

    let note = NotesRepo::new(&conn)
        .get(&id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("笔记不存在：{id}"))?;

    crate::retrieval::sync_note(&conn, &note.id, &note.title, &note.content, &note.tags)
        .map_err(|e| e.to_string())?;
    index_note_quietly(&conn, &state, &note);
    Ok(note)
}

/// 重新同步指定笔记的全文索引（标签变更后调用）
fn resync_notes(conn: &rusqlite::Connection, note_ids: &[String]) {
    let repo = NotesRepo::new(conn);
    for note_id in note_ids {
        match repo.get(note_id) {
            Ok(Some(note)) => {
                if let Err(err) =
                    crate::retrieval::sync_note(conn, &note.id, &note.title, &note.content, &note.tags)
                {
                    eprintln!("索引同步失败（note={}）：{err}", note.id);
                }
            }
            Ok(None) => {}
            Err(err) => eprintln!("读取笔记失败（note={note_id}）：{err}"),
        }
    }
}

/// 列出全部群组
#[tauri::command]
pub fn list_groups(state: State<'_, AppState>) -> Result<Vec<GroupDto>, String> {
    let conn = state.lock()?;
    GroupsRepo::new(&conn).list().map_err(|e| e.to_string())
}

/// 新建群组（parent_id 为 null 表示根级）
#[tauri::command]
pub fn create_group(
    state: State<'_, AppState>,
    name: String,
    parent_id: Option<String>,
) -> Result<GroupDto, String> {
    let conn = state.lock()?;
    GroupsRepo::new(&conn)
        .create(&name, parent_id.as_deref())
        .map_err(|e| e.to_string())
}

/// 重命名群组
#[tauri::command]
pub fn rename_group(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> Result<bool, String> {
    let conn = state.lock()?;
    GroupsRepo::new(&conn)
        .rename(&id, &name)
        .map_err(|e| e.to_string())
}

/// 删除群组（子群组级联删除，组内笔记转为未分组）
#[tauri::command]
pub fn delete_group(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    let conn = state.lock()?;
    GroupsRepo::new(&conn).delete(&id).map_err(|e| e.to_string())
}

/// 移动群组到新父级（拒绝环路）
#[tauri::command]
pub fn move_group(
    state: State<'_, AppState>,
    id: String,
    parent_id: Option<String>,
) -> Result<bool, String> {
    let conn = state.lock()?;
    GroupsRepo::new(&conn)
        .relocate(&id, parent_id.as_deref())
        .map_err(|e| e.to_string())
}

/// 按给定顺序重排同级群组
#[tauri::command]
pub fn reorder_groups(
    state: State<'_, AppState>,
    ordered_ids: Vec<String>,
) -> Result<(), String> {
    let conn = state.lock()?;
    GroupsRepo::new(&conn)
        .reorder(&ordered_ids)
        .map_err(|e| e.to_string())
}

/// 导入 Markdown 文件或目录（支持单文件、文件夹批量、拖拽路径）
#[tauri::command]
pub fn import_markdown(
    state: State<'_, AppState>,
    paths: Vec<String>,
    group_id: Option<String>,
) -> Result<ImportOutcome, String> {
    if paths.is_empty() {
        return Ok(ImportOutcome { imported: 0, skipped: Vec::new(), notes: Vec::new() });
    }
    let inputs: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let conn = state.lock()?;
    let outcome = crate::knowledge::import_paths(&conn, &inputs, group_id.as_deref())
        .map_err(|e| e.to_string())?;
    // 导入后建立向量索引，便于语义检索立即命中
    for note in &outcome.notes {
        index_note_quietly(&conn, &state, note);
    }
    Ok(outcome)
}
