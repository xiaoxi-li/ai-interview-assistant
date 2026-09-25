//! 数据导出与备份：
//! * Markdown 导出（保留 front matter 中的标签）
//! * JSON 逻辑备份与恢复
//! * 数据库文件级备份（VACUUM INTO，保证副本一致）

use super::store::{now_ms, GroupsRepo, NotesRepo, TagsRepo};
use super::{GroupDto, NoteDto, TagDto};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 备份格式版本
pub const BACKUP_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSummary {
    /// 导出的文件数（数据库备份固定为 1）
    pub files: usize,
    pub dest: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreSummary {
    pub groups: usize,
    pub tags: usize,
    pub notes: usize,
}

/// 完整备份载荷
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupPayload {
    pub version: u32,
    pub exported_at: i64,
    pub groups: Vec<GroupDto>,
    pub tags: Vec<TagDto>,
    pub notes: Vec<NoteDto>,
}

fn io_err(err: std::io::Error) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(err))
}

/// 文件名安全化：替换 Windows/macOS 非法字符
pub fn sanitize_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if (c as u32) < 0x20 => '_',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim().trim_end_matches('.').to_string();
    if trimmed.is_empty() {
        "未命名".to_string()
    } else {
        trimmed
    }
}

fn yaml_scalar(value: &str) -> String {
    if value.contains([':', '#', '"', '\'', '\n']) {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

/// 单篇笔记导出为 Markdown（含 YAML front matter）
pub fn note_to_markdown(note: &NoteDto) -> String {
    let mut out = String::from("---\n");
    out.push_str(&format!("title: {}\n", yaml_scalar(&note.title)));
    if !note.tags.is_empty() {
        out.push_str("tags:\n");
        for tag in &note.tags {
            out.push_str(&format!("  - {}\n", yaml_scalar(tag)));
        }
    }
    out.push_str(&format!("updatedAt: {}\n---\n\n", note.updated_at));

    // 正文已含标题时避免重复插入
    if !note.content.trim_start().starts_with('#') {
        out.push_str(&format!("# {}\n\n", note.title));
    }
    out.push_str(&note.content);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// 依据群组层级构建输出目录
fn group_dir(dest: &Path, group_id: Option<&str>, groups: &[GroupDto]) -> PathBuf {
    let Some(mut current) = group_id.map(str::to_string) else {
        return dest.to_path_buf();
    };

    let mut parts: Vec<String> = Vec::new();
    // 限制遍历深度，避免异常数据造成死循环
    for _ in 0..=groups.len() {
        let Some(group) = groups.iter().find(|g| g.id == current) else {
            break;
        };
        parts.push(sanitize_file_name(&group.name));
        match group.parent_id.clone() {
            Some(parent) => current = parent,
            None => break,
        }
    }
    parts.reverse();

    let mut path = dest.to_path_buf();
    for part in parts {
        path.push(part);
    }
    path
}

/// 导出全部笔记到目录（按群组层级分文件夹）
pub fn export_notes_to_dir(conn: &Connection, dest: &Path) -> rusqlite::Result<ExportSummary> {
    let notes = NotesRepo::new(conn).list(true)?;
    let groups = GroupsRepo::new(conn).list()?;

    std::fs::create_dir_all(dest).map_err(io_err)?;

    let mut files = 0usize;
    for note in &notes {
        let dir = group_dir(dest, note.group_id.as_deref(), &groups);
        std::fs::create_dir_all(&dir).map_err(io_err)?;
        let file = dir.join(format!("{}.md", sanitize_file_name(&note.title)));
        std::fs::write(&file, note_to_markdown(note)).map_err(io_err)?;
        files += 1;
    }

    Ok(ExportSummary {
        files,
        dest: dest.display().to_string(),
    })
}

/// 构建完整备份载荷
pub fn build_backup(conn: &Connection) -> rusqlite::Result<BackupPayload> {
    Ok(BackupPayload {
        version: BACKUP_VERSION,
        exported_at: now_ms(),
        groups: GroupsRepo::new(conn).list()?,
        tags: TagsRepo::new(conn).list()?,
        notes: NotesRepo::new(conn).list(true)?,
    })
}

/// 导出 JSON 备份到指定文件
pub fn export_backup_json(conn: &Connection, dest: &Path) -> rusqlite::Result<ExportSummary> {
    let payload = build_backup(conn)?;
    let json = serde_json::to_string_pretty(&payload)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(io_err)?;
        }
    }
    std::fs::write(dest, json).map_err(io_err)?;
    Ok(ExportSummary {
        files: payload.notes.len(),
        dest: dest.display().to_string(),
    })
}

/// 从 JSON 备份恢复（按 ID 去重，已存在的记录跳过）
pub fn restore_backup_json(conn: &Connection, src: &Path) -> rusqlite::Result<RestoreSummary> {
    let raw = std::fs::read_to_string(src).map_err(io_err)?;
    let payload: BackupPayload = serde_json::from_str(&raw)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

    let mut summary = RestoreSummary {
        groups: 0,
        tags: 0,
        notes: 0,
    };

    // 标签：按名称复用
    let tags_repo = TagsRepo::new(conn);
    for tag in &payload.tags {
        let before = tags_repo.list()?.len();
        tags_repo.ensure(&tag.name)?;
        if tags_repo.list()?.len() > before {
            summary.tags += 1;
        }
    }

    // 群组：父级优先插入（多轮推进，孤儿降级为根）
    let mut pending = payload.groups.clone();
    let mut guard = 0usize;
    while !pending.is_empty() && guard <= payload.groups.len() + 1 {
        guard += 1;
        let mut rest = Vec::new();
        let mut progressed = false;
        for group in pending {
            let exists: Option<String> = conn
                .query_row(
                    "SELECT id FROM groups WHERE id = ?1",
                    params![group.id],
                    |r| r.get(0),
                )
                .ok();
            if exists.is_some() {
                progressed = true;
                continue;
            }
            let parent_ok = match group.parent_id.as_deref() {
                None => true,
                Some(parent) => conn
                    .query_row(
                        "SELECT id FROM groups WHERE id = ?1",
                        params![parent],
                        |r| r.get::<_, String>(0),
                    )
                    .is_ok(),
            };
            if !parent_ok {
                rest.push(group);
                continue;
            }
            conn.execute(
                "INSERT INTO groups (id, name, parent_id, sort_order, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                params![group.id, group.name, group.parent_id, group.order, now_ms()],
            )?;
            summary.groups += 1;
            progressed = true;
        }
        if rest.is_empty() {
            break;
        }
        if !progressed {
            // 父级缺失：降级为根节点后插入
            for group in rest {
                conn.execute(
                    "INSERT OR IGNORE INTO groups (id, name, parent_id, sort_order, created_at, updated_at)
                     VALUES (?1, ?2, NULL, ?3, ?4, ?4)",
                    params![group.id, group.name, group.order, now_ms()],
                )?;
                summary.groups += 1;
            }
            break;
        }
        pending = rest;
    }

    // 笔记
    for note in &payload.notes {
        let exists: Option<String> = conn
            .query_row("SELECT id FROM notes WHERE id = ?1", params![note.id], |r| {
                r.get(0)
            })
            .ok();
        if exists.is_some() {
            continue;
        }
        conn.execute(
            "INSERT INTO notes (id, title, content, group_id, is_deleted, deleted_at, created_at, updated_at)
             VALUES (?1, ?2, ?3, NULL, ?4, NULL, ?5, ?6)",
            params![
                note.id,
                note.title,
                note.content,
                note.is_deleted as i64,
                note.created_at,
                note.updated_at
            ],
        )?;
        // 群组可能已被降级处理，逐个校验存在性
        if let Some(group_id) = note.group_id.as_deref() {
            let group_exists = conn
                .query_row(
                    "SELECT id FROM groups WHERE id = ?1",
                    params![group_id],
                    |r| r.get::<_, String>(0),
                )
                .is_ok();
            if group_exists {
                conn.execute(
                    "UPDATE notes SET group_id = ?2 WHERE id = ?1",
                    params![note.id, group_id],
                )?;
            }
        }
        if !note.tags.is_empty() {
            tags_repo.set_note_tags(&note.id, &note.tags)?;
        }
        summary.notes += 1;
    }

    Ok(summary)
}

/// 数据库文件级备份（生成一致副本）
pub fn backup_database_file(conn: &Connection, dest: &Path) -> rusqlite::Result<ExportSummary> {
    if dest.exists() {
        std::fs::remove_file(dest).map_err(io_err)?;
    }
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(io_err)?;
        }
    }
    conn.execute("VACUUM INTO ?1", params![dest.to_string_lossy()])?;
    Ok(ExportSummary {
        files: 1,
        dest: dest.display().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Db;
    use std::path::PathBuf;

    fn temp_dir(tag: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "aia-export-{tag}-{}-{}",
            std::process::id(),
            now_ms()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn markdown_contains_front_matter_and_tags() {
        let note = NoteDto {
            id: "n1".into(),
            title: "架构设计".into(),
            content: "正文内容".into(),
            group_id: None,
            tags: vec!["架构".into(), "Rust".into()],
            is_deleted: false,
            created_at: 1,
            updated_at: 2,
        };
        let md = note_to_markdown(&note);
        assert!(md.starts_with("---\n"));
        assert!(md.contains("title: 架构设计"));
        assert!(md.contains("  - 架构"));
        assert!(md.contains("# 架构设计"), "缺少标题时应补一级标题");
        assert!(md.contains("正文内容"));
    }

    #[test]
    fn markdown_does_not_duplicate_existing_heading() {
        let note = NoteDto {
            id: "n1".into(),
            title: "标题".into(),
            content: "# 已有标题\n正文".into(),
            group_id: None,
            tags: vec![],
            is_deleted: false,
            created_at: 1,
            updated_at: 2,
        };
        let md = note_to_markdown(&note);
        assert_eq!(md.matches("# 已有标题").count(), 1);
    }

    #[test]
    fn sanitize_replaces_illegal_characters() {
        assert_eq!(sanitize_file_name("a/b:c*d?"), "a_b_c_d_");
        assert_eq!(sanitize_file_name("   "), "未命名");
        assert_eq!(sanitize_file_name("正常名称"), "正常名称");
    }

    #[test]
    fn export_writes_files_grouped_by_folder() {
        let dir = temp_dir("md");
        let db = Db::open_in_memory().expect("db");
        let groups = GroupsRepo::new(db.conn());
        let parent = groups.create("前端", None).expect("parent");
        let child = groups.create("React", Some(&parent.id)).expect("child");
        let notes = NotesRepo::new(db.conn());
        notes.create("笔记一", "内容一", Some(&child.id)).expect("note1");
        notes.create("笔记二", "内容二", None).expect("note2");

        let summary = export_notes_to_dir(db.conn(), &dir).expect("export");
        assert_eq!(summary.files, 2);
        assert!(dir.join("前端/React/笔记一.md").exists());
        assert!(dir.join("笔记二.md").exists());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn json_backup_round_trip_restores_notes() {
        let dir = temp_dir("json");
        let file = dir.join("backup.json");
        let source = Db::open_in_memory().expect("source db");
        {
            let notes = NotesRepo::new(source.conn());
            let note = notes.create("备份笔记", "备份内容", None).expect("create");
            TagsRepo::new(source.conn())
                .set_note_tags(&note.id, &["架构".to_string()])
                .expect("tags");
        }
        let summary = export_backup_json(source.conn(), &file).expect("export");
        assert_eq!(summary.files, 1);

        let target = Db::open_in_memory().expect("target db");
        let restored = restore_backup_json(target.conn(), &file).expect("restore");
        assert_eq!(restored.notes, 1);

        let notes = NotesRepo::new(target.conn()).list(false).expect("list");
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].title, "备份笔记");
        assert_eq!(notes[0].tags, vec!["架构".to_string()]);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn restore_skips_existing_records() {
        let dir = temp_dir("dedupe");
        let file = dir.join("backup.json");
        let db = Db::open_in_memory().expect("db");
        run_full_setup(&db, &file);

        let first = restore_backup_json(db.conn(), &file).expect("restore 1");
        assert!(first.notes + first.groups > 0);
        let second = restore_backup_json(db.conn(), &file).expect("restore 2");
        assert_eq!(second.notes, 0);
        assert_eq!(second.groups, 0);

        std::fs::remove_dir_all(&dir).ok();
    }

    fn run_full_setup(db: &Db, backup_path: &Path) {
        // 在另一库中构造数据并导出，再导入到当前库
        let other = Db::open_in_memory().expect("other db");
        let parent = GroupsRepo::new(other.conn())
            .create("父级", None)
            .expect("parent");
        let child = GroupsRepo::new(other.conn())
            .create("子级", Some(&parent.id))
            .expect("child");
        NotesRepo::new(other.conn())
            .create("笔记", "内容", Some(&child.id))
            .expect("note");
        export_backup_json(other.conn(), backup_path).expect("export");
        // 当前库需保持为空，这里仅断言可重复恢复
        let _ = db;
    }

    #[test]
    fn database_backup_file_can_be_opened() {
        let dir = temp_dir("db");
        let db_file = dir.join("backup.sqlite");
        let db = Db::open_in_memory().expect("db");
        NotesRepo::new(db.conn())
            .create("备份", "内容", None)
            .expect("create");

        let summary = backup_database_file(db.conn(), &db_file).expect("backup");
        assert_eq!(summary.files, 1);
        assert!(db_file.exists());

        // 备份文件应能被正常打开并读到数据
        let reopened = Db::open(&db_file).expect("reopen");
        let count = NotesRepo::new(reopened.conn()).count().expect("count");
        assert_eq!(count, 1);

        std::fs::remove_dir_all(&dir).ok();
    }
}
