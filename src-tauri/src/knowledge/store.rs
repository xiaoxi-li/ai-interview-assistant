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
        let note = NoteDto {
            id,
            title: title.to_string(),
            content: content.to_string(),
            group_id: group_id.map(str::to_string),
            tags: Vec::new(),
            is_deleted: false,
            created_at: now,
            updated_at: now,
        };
        // 同步全文索引（jieba 预分词后写入 FTS5）
        crate::retrieval::sync_note(self.conn, &note.id, &note.title, &note.content, &note.tags)?;
        Ok(note)
    }

    /// 更新笔记标题与正文；笔记不存在时返回 None
    pub fn update(&self, id: &str, title: &str, content: &str) -> Result<Option<NoteDto>> {
        let now = now_ms();
        let affected = self.conn.execute(
            "UPDATE notes SET title = ?2, content = ?3, updated_at = ?4 WHERE id = ?1",
            params![id, title, content, now],
        )?;
        if affected == 0 {
            return Ok(None);
        }
        let note = self.get(id)?;
        if let Some(current) = note.as_ref() {
            crate::retrieval::sync_note(
                self.conn,
                &current.id,
                &current.title,
                &current.content,
                &current.tags,
            )?;
        }
        Ok(note)
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

    /// 新建空白笔记
    pub fn create_empty(&self, group_id: Option<&str>) -> Result<NoteDto> {
        self.create("未命名笔记", "", group_id)
    }

    /// 移入回收站（软删除）；返回是否命中记录
    pub fn trash(&self, id: &str) -> Result<bool> {
        let affected = self.conn.execute(
            "UPDATE notes SET is_deleted = 1, deleted_at = ?2, updated_at = ?2
             WHERE id = ?1 AND is_deleted = 0",
            params![id, now_ms()],
        )?;
        if affected > 0 {
            crate::retrieval::remove_note(self.conn, id)?;
        }
        Ok(affected > 0)
    }

    /// 从回收站恢复；返回是否命中记录
    pub fn restore(&self, id: &str) -> Result<bool> {
        let affected = self.conn.execute(
            "UPDATE notes SET is_deleted = 0, deleted_at = NULL, updated_at = ?2
             WHERE id = ?1 AND is_deleted = 1",
            params![id, now_ms()],
        )?;
        if affected > 0 {
            // 恢复后重新建立全文索引
            if let Some(note) = self.get(id)? {
                crate::retrieval::sync_note(
                    self.conn,
                    &note.id,
                    &note.title,
                    &note.content,
                    &note.tags,
                )?;
            }
        }
        Ok(affected > 0)
    }

    /// 彻底删除单条笔记（级联清理标签关联、版本与分块）
    pub fn purge(&self, id: &str) -> Result<bool> {
        let affected = self
            .conn
            .execute("DELETE FROM notes WHERE id = ?1", params![id])?;
        if affected > 0 {
            crate::retrieval::prune_orphans(self.conn)?;
            crate::retrieval::index::prune_orphans(self.conn)?;
        }
        Ok(affected > 0)
    }

    /// 清空回收站，返回删除条数
    pub fn empty_trash(&self) -> Result<usize> {
        let affected = self
            .conn
            .execute("DELETE FROM notes WHERE is_deleted = 1", [])?;
        if affected > 0 {
            crate::retrieval::prune_orphans(self.conn)?;
            crate::retrieval::index::prune_orphans(self.conn)?;
        }
        Ok(affected)
    }

    /// 移动笔记到指定群组（None 表示移出群组）
    pub fn move_to_group(&self, id: &str, group_id: Option<&str>) -> Result<bool> {
        let affected = self.conn.execute(
            "UPDATE notes SET group_id = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, group_id, now_ms()],
        )?;
        Ok(affected > 0)
    }

    /// 列出回收站中的笔记（按删除时间倒序）
    pub fn list_trash(&self) -> Result<Vec<NoteDto>> {
        let sql =
            format!("SELECT {NOTE_COLUMNS} FROM notes WHERE is_deleted = 1 ORDER BY deleted_at DESC");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], map_note)?;
        let mut notes = Vec::new();
        for row in rows {
            notes.push(row?);
        }
        Ok(notes)
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

const GROUP_COLUMNS: &str = "id, name, parent_id, sort_order";

fn map_group(row: &rusqlite::Row<'_>) -> Result<GroupDto> {
    Ok(GroupDto {
        id: row.get(0)?,
        name: row.get(1)?,
        parent_id: row.get(2)?,
        order: row.get(3)?,
    })
}

impl<'a> GroupsRepo<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// 列出全部群组（按层级内的排序值）
    pub fn list(&self) -> Result<Vec<GroupDto>> {
        let sql = format!("SELECT {GROUP_COLUMNS} FROM groups ORDER BY sort_order, name");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], map_group)?;
        let mut groups = Vec::new();
        for row in rows {
            groups.push(row?);
        }
        Ok(groups)
    }

    pub fn get(&self, id: &str) -> Result<Option<GroupDto>> {
        let sql = format!("SELECT {GROUP_COLUMNS} FROM groups WHERE id = ?1");
        self.conn
            .query_row(&sql, params![id], map_group)
            .optional()
    }

    /// 新建群组，排序值自动追加到同级末尾
    pub fn create(&self, name: &str, parent_id: Option<&str>) -> Result<GroupDto> {
        let id = new_id("g");
        let now = now_ms();
        let order: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM groups WHERE parent_id IS ?1",
            params![parent_id],
            |row| row.get(0),
        )?;
        self.conn.execute(
            "INSERT INTO groups (id, name, parent_id, sort_order, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![id, name, parent_id, order, now],
        )?;
        Ok(GroupDto {
            id,
            name: name.to_string(),
            parent_id: parent_id.map(str::to_string),
            order,
        })
    }

    /// 重命名群组
    pub fn rename(&self, id: &str, name: &str) -> Result<bool> {
        let affected = self.conn.execute(
            "UPDATE groups SET name = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, name, now_ms()],
        )?;
        Ok(affected > 0)
    }

    /// 删除群组；子群组级联删除，组内笔记的 group_id 置空
    pub fn delete(&self, id: &str) -> Result<bool> {
        let affected = self
            .conn
            .execute("DELETE FROM groups WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    /// 收集指定群组的全部后代 ID（含自身），用于环路校验
    pub fn subtree_ids(&self, id: &str) -> Result<Vec<String>> {
        let mut collected = vec![id.to_string()];
        let mut frontier = vec![id.to_string()];
        while let Some(current) = frontier.pop() {
            let mut stmt = self
                .conn
                .prepare("SELECT id FROM groups WHERE parent_id = ?1")?;
            let rows = stmt.query_map(params![current], |row| row.get::<_, String>(0))?;
            for row in rows {
                let child = row?;
                collected.push(child.clone());
                frontier.push(child);
            }
        }
        Ok(collected)
    }

    /// 移动群组到新父级；拒绝移入自身或其后代（避免环路）
    // 注：方法名不能使用 `move`（Rust 关键字）
    pub fn relocate(&self, id: &str, parent_id: Option<&str>) -> Result<bool> {
        if let Some(target) = parent_id {
            if target == id || self.subtree_ids(id)?.iter().any(|x| x == target) {
                return Ok(false);
            }
        }
        let affected = self.conn.execute(
            "UPDATE groups SET parent_id = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, parent_id, now_ms()],
        )?;
        Ok(affected > 0)
    }

    /// 按给定顺序重排同级群组（数组下标即新的 sort_order）
    pub fn reorder(&self, ordered_ids: &[String]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        let now = now_ms();
        for (index, id) in ordered_ids.iter().enumerate() {
            tx.execute(
                "UPDATE groups SET sort_order = ?2, updated_at = ?3 WHERE id = ?1",
                params![id, index as i64, now],
            )?;
        }
        tx.commit()?;
        Ok(())
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

    #[test]
    fn update_changes_title_and_content() {
        with_db(|conn| {
            let repo = NotesRepo::new(conn);
            let created = repo.create("原标题", "原正文", None).expect("create");

            let updated = repo
                .update(&created.id, "新标题", "新正文\n\n| a | b |")
                .expect("update")
                .expect("exists");

            assert_eq!(updated.title, "新标题");
            assert_eq!(updated.content, "新正文\n\n| a | b |");
            assert_eq!(updated.created_at, created.created_at);
            assert!(updated.updated_at >= created.updated_at);
        });
    }

    #[test]
    fn update_unknown_id_returns_none() {
        with_db(|conn| {
            let repo = NotesRepo::new(conn);
            assert!(repo.update("missing", "t", "c").expect("update").is_none());
        });
    }

    #[test]
    fn create_empty_note_has_default_title() {
        with_db(|conn| {
            let repo = NotesRepo::new(conn);
            let note = repo.create_empty(None).expect("create");
            assert_eq!(note.title, "未命名笔记");
            assert!(note.content.is_empty());
        });
    }

    #[test]
    fn trash_moves_note_out_of_active_list() {
        with_db(|conn| {
            let repo = NotesRepo::new(conn);
            let note = repo.create("待删除", "内容", None).expect("create");

            assert!(repo.trash(&note.id).expect("trash"));
            assert_eq!(repo.list(false).expect("list").len(), 0);
            assert_eq!(repo.list_trash().expect("trash list").len(), 1);
            // 重复删除应返回 false
            assert!(!repo.trash(&note.id).expect("trash twice"));
        });
    }

    #[test]
    fn restore_returns_note_to_active_list() {
        with_db(|conn| {
            let repo = NotesRepo::new(conn);
            let note = repo.create("恢复我", "内容", None).expect("create");
            repo.trash(&note.id).expect("trash");

            assert!(repo.restore(&note.id).expect("restore"));
            assert_eq!(repo.list(false).expect("list").len(), 1);
            assert!(repo.list_trash().expect("trash list").is_empty());
            // 未在回收站的笔记不可恢复
            assert!(!repo.restore(&note.id).expect("restore twice"));
        });
    }

    #[test]
    fn purge_and_empty_trash_remove_records_permanently() {
        with_db(|conn| {
            let repo = NotesRepo::new(conn);
            let a = repo.create("A", "a", None).expect("create a");
            let b = repo.create("B", "b", None).expect("create b");
            repo.trash(&a.id).expect("trash a");
            repo.trash(&b.id).expect("trash b");

            assert!(repo.purge(&a.id).expect("purge"));
            assert!(repo.get(&a.id).expect("get").is_none());
            assert_eq!(repo.empty_trash().expect("empty"), 1);
            assert!(repo.get(&b.id).expect("get").is_none());
        });
    }

    #[test]
    fn move_note_updates_group_and_clears_it() {
        with_db(|conn| {
            let repo = NotesRepo::new(conn);
            conn.execute(
                "INSERT INTO groups (id, name, parent_id, sort_order, created_at, updated_at)
                 VALUES ('g1', '前端', NULL, 0, 0, 0)",
                [],
            )
            .expect("insert group");

            let note = repo.create("移动我", "内容", None).expect("create");
            assert!(repo.move_to_group(&note.id, Some("g1")).expect("move in"));
            assert_eq!(
                repo.get(&note.id).expect("get").expect("exists").group_id,
                Some("g1".to_string())
            );

            assert!(repo.move_to_group(&note.id, None).expect("move out"));
            assert_eq!(
                repo.get(&note.id).expect("get").expect("exists").group_id,
                None
            );
        });
    }

    #[test]
    fn group_create_assigns_incrementing_order() {
        with_db(|conn| {
            let repo = GroupsRepo::new(conn);
            let a = repo.create("前端", None).expect("create a");
            let b = repo.create("后端", None).expect("create b");
            let child = repo.create("React", Some(&a.id)).expect("create child");

            assert_eq!(a.order, 0);
            assert_eq!(b.order, 1);
            assert_eq!(child.order, 0, "子级排序独立计算");
            assert_eq!(child.parent_id.as_deref(), Some(a.id.as_str()));
        });
    }

    #[test]
    fn group_rename_then_delete_cascades_children_and_ungroups_notes() {
        with_db(|conn| {
            let groups = GroupsRepo::new(conn);
            let notes = NotesRepo::new(conn);
            let parent = groups.create("父级", None).expect("create parent");
            let child = groups.create("子级", Some(&parent.id)).expect("create child");
            let note = notes
                .create("笔记", "内容", Some(&child.id))
                .expect("create note");

            assert!(groups.rename(&parent.id, "新父级").expect("rename"));
            assert_eq!(
                groups.get(&parent.id).expect("get").expect("exists").name,
                "新父级"
            );

            assert!(groups.delete(&parent.id).expect("delete"));
            assert!(groups.get(&child.id).expect("get child").is_none(), "子群组应级联删除");
            assert_eq!(
                notes.get(&note.id).expect("get note").expect("exists").group_id,
                None,
                "删除群组后笔记应变为未分组"
            );
        });
    }

    #[test]
    fn group_move_rejects_cycles() {
        with_db(|conn| {
            let repo = GroupsRepo::new(conn);
            let a = repo.create("A", None).expect("a");
            let b = repo.create("B", Some(&a.id)).expect("b");
            let c = repo.create("C", Some(&b.id)).expect("c");

            assert!(!repo.relocate(&a.id, Some(&a.id)).expect("self"), "不能移入自身");
            assert!(
                !repo.relocate(&a.id, Some(&c.id)).expect("descendant"),
                "不能移入自己的后代"
            );
            assert!(repo.relocate(&c.id, None).expect("to root"));
            assert_eq!(repo.get(&c.id).expect("get").expect("exists").parent_id, None);
        });
    }

    #[test]
    fn subtree_ids_contains_self_and_descendants() {
        with_db(|conn| {
            let repo = GroupsRepo::new(conn);
            let a = repo.create("A", None).expect("a");
            let b = repo.create("B", Some(&a.id)).expect("b");
            let c = repo.create("C", Some(&b.id)).expect("c");

            let mut ids = repo.subtree_ids(&a.id).expect("subtree");
            ids.sort();
            let mut expected = vec![a.id.clone(), b.id.clone(), c.id.clone()];
            expected.sort();
            assert_eq!(ids, expected);
        });
    }

    #[test]
    fn group_reorder_applies_given_order() {
        with_db(|conn| {
            let repo = GroupsRepo::new(conn);
            let a = repo.create("A", None).expect("a");
            let b = repo.create("B", None).expect("b");
            let c = repo.create("C", None).expect("c");

            repo.reorder(&[c.id.clone(), a.id.clone(), b.id.clone()])
                .expect("reorder");

            let ordered: Vec<String> = repo
                .list()
                .expect("list")
                .into_iter()
                .map(|g| g.id)
                .collect();
            assert_eq!(ordered, vec![c.id, a.id, b.id]);
        });
    }
}
