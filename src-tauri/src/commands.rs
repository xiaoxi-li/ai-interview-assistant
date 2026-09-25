//! Tauri 命令：前端调用入口。
//!
//! 约定：所有命令返回 `Result<T, String>`，错误直接以字符串形式透传给前端。

use crate::knowledge::export::{ExportSummary, RestoreSummary};
use crate::knowledge::store::{GroupsRepo, NotesRepo, TagsRepo};
use crate::knowledge::{GroupDto, ImportOutcome, NoteDto, TagDto};
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
    let note = NotesRepo::new(&conn)
        .update(&id, &title, &content)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("笔记不存在：{id}"))?;
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
