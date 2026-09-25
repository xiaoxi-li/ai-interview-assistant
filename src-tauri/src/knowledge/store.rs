//! 知识库数据访问：笔记与群组仓储。

use crate::knowledge::{GroupDto, NoteDto};
use rusqlite::{params, Connection, OptionalExtension, Result};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// 毫秒级时间戳
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 生成唯一 ID（时间戳 + 纳秒 + 进程内自增，避免引入 ULID 依赖）
pub fn new_id(prefix: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    format!("{prefix}{:x}{:x}{:x}", now_ms(), nanos, n)
}

const NOTE_COLUMNS: &str = "id, title, content, group_id, is_deleted, created_at, updated_at";

fn map_note(row: &rusqlite::Row<'_>) -> Result<NoteDto> {
    Ok(NoteDto {
        id: row.get(0)?,
        title: row.get(1)?,
        content: row.get(2)?,
        group_id: row.get(3)?,
        tags: Vec::new(),
        is_deleted: row.get::<_, i64>(4)? != 0,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

/// 笔记仓储
pub struct NotesRepo<'a> {
    conn: &'a Connection,
}

impl<'a> NotesRepo<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// 新建笔记
    pub fn create(&self, title: &str, content: &str, group_id: Option<&str>) -> Result<NoteDto> {
        let now = now_ms();
        let id = new_id("n");
        self.conn.execute(
            "INSERT INTO notes (id, title, content, group_id, is_deleted, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?5, ?5)",
            params![id, title, content, group_id, now],
        )?;
        Ok(NoteDto {
            id,
            title: title.to_string(),
            content: content.to_string(),
            group_id: group_id.map(str::to_string),
            tags: Vec::new(),
            is_deleted: false,
            created_at: now,
            updated_at: now,
        })
    }

    /// 按 ID 查询（含回收站中的笔记）
    pub fn get(&self, id: &str) -> Result<Option<NoteDto>> {
        let sql = format!("SELECT {NOTE_COLUMNS} FROM notes WHERE id = ?1");
        let mut note = self
            .conn
            .query_row(&sql, params![id], map_note)
            .optional()?;
        if let Some(n) = note.as_mut() {
            n.tags = self.tags_of(&n.id)?;
        }
        Ok(note)
    }

    /// 列出笔记（按更新时间倒序）
    pub fn list(&self, include_deleted: bool) -> Result<Vec<NoteDto>> {
        let sql = if include_deleted {
            format!("SELECT {NOTE_COLUMNS} FROM notes ORDER BY updated_at DESC")
        } else {
            format!(
                "SELECT {NOTE_COLUMNS} FROM notes WHERE is_deleted = 0 ORDER BY updated_at DESC"
            )
        };
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], map_note)?;
        let mut notes = Vec::new();
        for row in rows {
            notes.push(row?);
        }

        let tag_map = self.all_tags()?;
        for note in &mut notes {
            if let Some(tags) = tag_map.get(&note.id) {
                note.tags = tags.clone();
            }
        }
        Ok(notes)
    }

    /// 笔记总数（不含回收站）
    pub fn count(&self) -> Result<usize> {
        let n: i64 = self.conn.query_row(
            "SELECT count(*) FROM notes WHERE is_deleted = 0",
            [],
            |row| row.get(0),
        )?;
        Ok(n as usize)
    }

    fn tags_of(&self, note_id: &str) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.name FROM note_tags nt JOIN tags t ON t.id = nt.tag_id
             WHERE nt.note_id = ?1 ORDER BY t.name",
        )?;
        let rows = stmt.query_map(params![note_id], |row| row.get::<_, String>(0))?;
        let mut tags = Vec::new();
        for row in rows {
            tags.push(row?);
        }
        Ok(tags)
    }

    fn all_tags(&self) -> Result<HashMap<String, Vec<String>>> {
        let mut stmt = self.conn.prepare(
            "SELECT nt.note_id, t.name FROM note_tags nt
             JOIN tags t ON t.id = nt.tag_id ORDER BY t.name",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        for row in rows {
            let (note_id, tag) = row?;
            map.entry(note_id).or_default().push(tag);
        }
        Ok(map)
    }
}

/// 群组仓储
pub struct GroupsRepo<'a> {
    conn: &'a Connection,
}

impl<'a> GroupsRepo<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn list(&self) -> Result<Vec<GroupDto>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, parent_id, sort_order FROM groups ORDER BY sort_order, name")?;
        let rows = stmt.query_map([], |row| {
            Ok(GroupDto {
                id: row.get(0)?,
                name: row.get(1)?,
                parent_id: row.get(2)?,
                order: row.get(3)?,
            })
        })?;
        let mut groups = Vec::new();
        for row in rows {
            groups.push(row?);
        }
        Ok(groups)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Db;

    fn with_db<T>(f: impl FnOnce(&Connection) -> T) -> T {
        let db = Db::open_in_memory().expect("db");
        f(db.conn())
    }

    #[test]
    fn create_then_get_round_trip() {
        with_db(|conn| {
            let repo = NotesRepo::new(conn);
            let created = repo.create("标题", "正文内容", None).expect("create");
            assert!(!created.id.is_empty());
            assert_eq!(created.created_at, created.updated_at);

            let fetched = repo.get(&created.id).expect("get").expect("exists");
            assert_eq!(fetched.title, "标题");
            assert_eq!(fetched.content, "正文内容");
            assert!(!fetched.is_deleted);
        });
    }

    #[test]
    fn list_orders_by_updated_desc_and_excludes_deleted() {
        with_db(|conn| {
            let repo = NotesRepo::new(conn);
            let first = repo.create("第一篇", "a", None).expect("create");
            let second = repo.create("第二篇", "b", None).expect("create");

            conn.execute(
                "UPDATE notes SET updated_at = 1 WHERE id = ?1",
                params![first.id],
            )
            .expect("bump");

            let notes = repo.list(false).expect("list");
            assert_eq!(notes.len(), 2);
            assert_eq!(notes[0].id, second.id, "应按更新时间倒序");

            conn.execute(
                "UPDATE notes SET is_deleted = 1 WHERE id = ?1",
                params![second.id],
            )
            .expect("soft delete");
            assert_eq!(repo.list(false).expect("list").len(), 1);
            assert_eq!(repo.list(true).expect("list all").len(), 2);
            assert_eq!(repo.count().expect("count"), 1);
        });
    }

    #[test]
    fn get_returns_none_for_unknown_id() {
        with_db(|conn| {
            let repo = NotesRepo::new(conn);
            assert!(repo.get("nope").expect("get").is_none());
        });
    }

    #[test]
    fn ids_are_unique() {
        with_db(|conn| {
            let repo = NotesRepo::new(conn);
            let mut ids = std::collections::HashSet::new();
            for _ in 0..200 {
                let n = repo.create("t", "c", None).expect("create");
                assert!(ids.insert(n.id), "ID 出现重复");
            }
        });
    }
}
