//! 转写持久化：面试会话与转写段落（P0 要求转写实时落库，崩溃不丢数据）。

use crate::knowledge::store::{new_id, now_ms};
use crate::providers::types::Speaker;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

/// 面试会话
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDto {
    pub id: String,
    pub title: String,
    pub started_at: i64,
    pub ended_at: Option<i64>,
}

/// 转写段落
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSegmentDto {
    pub id: String,
    pub session_id: String,
    /// `me` / `interviewer`
    pub speaker: String,
    pub text: String,
    pub is_final: bool,
    pub ts: i64,
}

const SEGMENT_COLUMNS: &str = "id, session_id, speaker, text, is_final, ts";
const SESSION_COLUMNS: &str = "id, title, started_at, ended_at";

fn map_segment(row: &rusqlite::Row<'_>) -> rusqlite::Result<TranscriptSegmentDto> {
    Ok(TranscriptSegmentDto {
        id: row.get(0)?,
        session_id: row.get(1)?,
        speaker: row.get(2)?,
        text: row.get(3)?,
        is_final: row.get::<_, i64>(4)? != 0,
        ts: row.get(5)?,
    })
}

fn map_session(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionDto> {
    Ok(SessionDto {
        id: row.get(0)?,
        title: row.get(1)?,
        started_at: row.get(2)?,
        ended_at: row.get(3)?,
    })
}

/// 转写仓储
pub struct TranscriptRepo<'a> {
    conn: &'a Connection,
}

impl<'a> TranscriptRepo<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// 新建会话（标题为空时使用默认标题）
    pub fn start_session(&self, title: &str) -> rusqlite::Result<SessionDto> {
        let session = SessionDto {
            id: new_id("s"),
            title: if title.trim().is_empty() {
                "面试会话".to_string()
            } else {
                title.trim().to_string()
            },
            started_at: now_ms(),
            ended_at: None,
        };
        self.conn.execute(
            "INSERT INTO sessions (id, title, started_at, ended_at) VALUES (?1, ?2, ?3, NULL)",
            params![session.id, session.title, session.started_at],
        )?;
        Ok(session)
    }

    /// 结束会话
    pub fn end_session(&self, id: &str) -> rusqlite::Result<bool> {
        let affected = self.conn.execute(
            "UPDATE sessions SET ended_at = ?2 WHERE id = ?1 AND ended_at IS NULL",
            params![id, now_ms()],
        )?;
        Ok(affected > 0)
    }

    pub fn get_session(&self, id: &str) -> rusqlite::Result<Option<SessionDto>> {
        let sql = format!("SELECT {SESSION_COLUMNS} FROM sessions WHERE id = ?1");
        self.conn.query_row(&sql, params![id], map_session).optional()
    }

    /// 最近一个未结束的会话（用于崩溃后恢复/续写）
    pub fn latest_open_session(&self) -> rusqlite::Result<Option<SessionDto>> {
        let sql = format!(
            "SELECT {SESSION_COLUMNS} FROM sessions WHERE ended_at IS NULL
             ORDER BY started_at DESC, rowid DESC LIMIT 1"
        );
        self.conn.query_row(&sql, [], map_session).optional()
    }

    /// 追加一条转写（partial 与 final 均可落库，便于崩溃恢复）
    pub fn append(
        &self,
        session_id: &str,
        speaker: Speaker,
        text: &str,
        is_final: bool,
        ts: i64,
    ) -> rusqlite::Result<String> {
        let id = new_id("t");
        self.conn.execute(
            "INSERT INTO transcript_segments (id, session_id, speaker, text, is_final, ts)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id,
                session_id,
                speaker.as_str(),
                text,
                if is_final { 1 } else { 0 },
                ts
            ],
        )?;
        Ok(id)
    }

    /// 读取会话的转写段落（按时间正序）
    pub fn list_segments(
        &self,
        session_id: &str,
        limit: usize,
    ) -> rusqlite::Result<Vec<TranscriptSegmentDto>> {
        let sql = format!(
            "SELECT {SEGMENT_COLUMNS} FROM transcript_segments WHERE session_id = ?1
             ORDER BY ts ASC, rowid ASC LIMIT ?2"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![session_id, limit as i64], map_segment)?;
        let mut segments = Vec::new();
        for row in rows {
            segments.push(row?);
        }
        Ok(segments)
    }

    /// 仅读取最终结果（final）
    pub fn list_finals(&self, session_id: &str) -> rusqlite::Result<Vec<TranscriptSegmentDto>> {
        let sql = format!(
            "SELECT {SEGMENT_COLUMNS} FROM transcript_segments
             WHERE session_id = ?1 AND is_final = 1 ORDER BY ts ASC, rowid ASC"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![session_id], map_segment)?;
        let mut segments = Vec::new();
        for row in rows {
            segments.push(row?);
        }
        Ok(segments)
    }

    pub fn count_segments(&self, session_id: &str) -> rusqlite::Result<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT count(*) FROM transcript_segments WHERE session_id = ?1",
            params![session_id],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    /// 仅统计最终结果条数（会话列表展示用）
    pub fn count_finals(&self, session_id: &str) -> rusqlite::Result<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT count(*) FROM transcript_segments WHERE session_id = ?1 AND is_final = 1",
            params![session_id],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Db;

    #[test]
    fn session_lifecycle_start_end() {
        let db = Db::open_in_memory().expect("db");
        let repo = TranscriptRepo::new(db.conn());

        let session = repo.start_session("技术面").expect("start");
        assert_eq!(session.title, "技术面");
        assert!(session.ended_at.is_none());

        let open = repo.latest_open_session().expect("open").expect("exists");
        assert_eq!(open.id, session.id);

        assert!(repo.end_session(&session.id).expect("end"));
        assert!(repo.latest_open_session().expect("open").is_none());
        // 重复结束返回 false
        assert!(!repo.end_session(&session.id).expect("end twice"));
        assert!(repo.get_session(&session.id).expect("get").expect("exists").ended_at.is_some());
    }

    #[test]
    fn blank_title_falls_back_to_default() {
        let db = Db::open_in_memory().expect("db");
        let repo = TranscriptRepo::new(db.conn());
        assert_eq!(
            repo.start_session("   ").expect("start").title,
            "面试会话"
        );
    }

    #[test]
    fn append_and_list_segments_keeps_order_and_speaker() {
        let db = Db::open_in_memory().expect("db");
        let repo = TranscriptRepo::new(db.conn());
        let session = repo.start_session("面试").expect("start");

        repo.append(&session.id, Speaker::Interviewer, "请介绍一下项目", true, 1000)
            .expect("append 1");
        repo.append(&session.id, Speaker::Interviewer, "请介绍", false, 900)
            .expect("append partial");
        repo.append(&session.id, Speaker::Me, "好的", true, 2000)
            .expect("append 2");

        let all = repo.list_segments(&session.id, 10).expect("list");
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].ts, 900, "应按时间正序");
        assert!(!all[0].is_final);

        let finals = repo.list_finals(&session.id).expect("finals");
        assert_eq!(finals.len(), 2);
        assert_eq!(finals[0].speaker, "interviewer");
        assert_eq!(finals[1].speaker, "me");
        assert_eq!(repo.count_segments(&session.id).expect("count"), 3);
    }

    #[test]
    fn list_segments_respects_limit() {
        let db = Db::open_in_memory().expect("db");
        let repo = TranscriptRepo::new(db.conn());
        let session = repo.start_session("面试").expect("start");

        for index in 0..5 {
            repo.append(&session.id, Speaker::Me, "文本", true, index)
                .expect("append");
        }
        assert_eq!(repo.list_segments(&session.id, 2).expect("list").len(), 2);
    }

    #[test]
    fn segments_are_scoped_by_session() {
        let db = Db::open_in_memory().expect("db");
        let repo = TranscriptRepo::new(db.conn());
        let a = repo.start_session("A").expect("a");
        let b = repo.start_session("B").expect("b");

        repo.append(&a.id, Speaker::Me, "A 的转写", true, 1).expect("append a");
        repo.append(&b.id, Speaker::Me, "B 的转写", true, 2).expect("append b");

        assert_eq!(repo.count_segments(&a.id).expect("count a"), 1);
        assert_eq!(repo.list_segments(&b.id, 10).expect("list b")[0].text, "B 的转写");
    }

    #[test]
    fn deleting_session_cascades_segments() {
        let db = Db::open_in_memory().expect("db");
        let repo = TranscriptRepo::new(db.conn());
        let session = repo.start_session("面试").expect("start");
        repo.append(&session.id, Speaker::Interviewer, "问题", true, 1)
            .expect("append");

        db.conn()
            .execute("DELETE FROM sessions WHERE id = ?1", params![session.id])
            .expect("delete session");

        assert_eq!(repo.count_segments(&session.id).expect("count"), 0);
    }

    #[test]
    fn append_rejects_unknown_session() {
        let db = Db::open_in_memory().expect("db");
        let repo = TranscriptRepo::new(db.conn());
        // 外键约束应拒绝孤儿段落
        assert!(repo
            .append("missing-session", Speaker::Me, "文本", true, 1)
            .is_err());
    }
}
