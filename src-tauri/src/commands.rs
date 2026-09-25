//! Tauri 命令：前端调用入口。
//!
//! 约定：所有命令返回 `Result<T, String>`，错误直接以字符串形式透传给前端。

use crate::knowledge::store::{GroupsRepo, NotesRepo};
use crate::knowledge::{GroupDto, ImportOutcome, NoteDto};
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
    NotesRepo::new(&conn)
        .update(&id, &title, &content)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("笔记不存在：{id}"))
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

/// 新建空白笔记
#[tauri::command]
pub fn create_note(
    state: State<'_, AppState>,
    group_id: Option<String>,
) -> Result<NoteDto, String> {
    let conn = state.lock()?;
    NotesRepo::new(&conn)
        .create_empty(group_id.as_deref())
        .map_err(|e| e.to_string())
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
    crate::knowledge::import_paths(&conn, &inputs, group_id.as_deref())
        .map_err(|e| e.to_string())
}
