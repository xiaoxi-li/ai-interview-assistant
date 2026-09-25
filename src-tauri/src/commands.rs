//! Tauri 命令：前端调用入口。
//!
//! 约定：所有命令返回 `Result<T, String>`，错误直接以字符串形式透传给前端。

use crate::knowledge::store::{GroupsRepo, NotesRepo};
use crate::knowledge::{GroupDto, ImportOutcome, NoteDto};
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

/// 列出全部群组
#[tauri::command]
pub fn list_groups(state: State<'_, AppState>) -> Result<Vec<GroupDto>, String> {
    let conn = state.lock()?;
    GroupsRepo::new(&conn).list().map_err(|e| e.to_string())
}

/// 导入 Markdown 文件或目录（支持单文件、文件夹批量、拖拽路径）
#[tauri::command]
pub fn import_markdown(
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> Result<ImportOutcome, String> {
    if paths.is_empty() {
        return Ok(ImportOutcome { imported: 0, skipped: Vec::new(), notes: Vec::new() });
    }
    let inputs: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let conn = state.lock()?;
    crate::knowledge::import_paths(&conn, &inputs, None).map_err(|e| e.to_string())
}
