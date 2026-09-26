//! Tauri 命令：前端调用入口。
//!
//! 约定：所有命令返回 `Result<T, String>`，错误直接以字符串形式透传给前端。

use crate::knowledge::export::{ExportSummary, RestoreSummary};
use crate::knowledge::store::{GroupsRepo, NotesRepo, TagsRepo, VersionsRepo};
use crate::knowledge::{GroupDto, ImportOutcome, NoteDto, NoteVersionDto, TagDto};
use crate::providers::AiProvider;
use crate::retrieval::SearchHit;
use crate::state::AppState;
use std::path::PathBuf;
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
