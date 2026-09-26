//! 问答持久化：识别出的问题与 AI 回答（`qa_pairs`）。

use crate::knowledge::store::new_id;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

/// 问答对
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QaPairDto {
    pub id: String,
    pub session_id: String,
    pub question: String,
    pub answer: String,
    /// 引用来源（笔记标题或 ID 列表）
    pub sources: Vec<String>,
    pub ts: i64,
}

const QA_COLUMNS: &str = "id, session_id, question, answer, sources_json, ts";

fn map_qa(row: &rusqlite::Row<'_>) -> rusqlite::Result<QaPairDto> {
    let sources_json: String = row.get(4)?;
    Ok(QaPairDto {
        id: row.get(0)?,
        session_id: row.get(1)?,
        question: row.get(2)?,
        answer: row.get(3)?,
        sources: serde_json::from_str(&sources_json).unwrap_or_default(),
        ts: row.get(5)?,
    })
}

/// 问答仓储
pub struct QaRepo<'a> {
    conn: &'a Connection,
}

impl<'a> QaRepo<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// 记录一个识别出的问题（回答留空，等待 IA-08 生成）
    pub fn create_question(
        &self,
        session_id: &str,
        question: &str,
        ts: i64,
    ) -> rusqlite::Result<QaPairDto> {
        let pair = QaPairDto {
            id: new_id("q"),
            session_id: session_id.to_string(),
            question: question.trim().to_string(),
            answer: String::new(),
            sources: Vec::new(),
            ts,
        };

        self.conn.execute(
            "INSERT INTO qa_pairs (id, session_id, question, answer, sources_json, ts)
             VALUES (?1, ?2, ?3, ?4, '[]', ?5)",
            params![pair.id, pair.session_id, pair.question, pair.answer, pair.ts],
        )?;

        Ok(pair)
    }

    pub fn get(&self, id: &str) -> rusqlite::Result<Option<QaPairDto>> {
        let sql = format!("SELECT {QA_COLUMNS} FROM qa_pairs WHERE id = ?1");
        self.conn.query_row(&sql, params![id], map_qa).optional()
    }

    /// 最近一次识别出的问题
    pub fn latest(&self, session_id: &str) -> rusqlite::Result<Option<QaPairDto>> {
        let sql = format!(
            "SELECT {QA_COLUMNS} FROM qa_pairs WHERE session_id = ?1 ORDER BY ts DESC, rowid DESC LIMIT 1"
        );
        self.conn.query_row(&sql, params![session_id], map_qa).optional()
    }

    /// 会话内的问题列表（按时间正序）
    pub fn list(&self, session_id: &str, limit: usize) -> rusqlite::Result<Vec<QaPairDto>> {
        let sql = format!(
            "SELECT {QA_COLUMNS} FROM qa_pairs WHERE session_id = ?1
             ORDER BY ts ASC, rowid ASC LIMIT ?2"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![session_id, limit as i64], map_qa)?;
        let mut pairs = Vec::new();
        for row in rows {
            pairs.push(row?);
        }
        Ok(pairs)
    }

    /// 写入/更新回答与引用来源
    pub fn set_answer(
        &self,
        id: &str,
        answer: &str,
        sources: &[String],
    ) -> rusqlite::Result<bool> {
        let sources_json = serde_json::to_string(sources).unwrap_or_else(|_| "[]".to_string());
        let affected = self.conn.execute(
            "UPDATE qa_pairs SET answer = ?2, sources_json = ?3 WHERE id = ?1",
            params![id, answer, sources_json],
        )?;
        Ok(affected > 0)
    }

    pub fn count(&self, session_id: &str) -> rusqlite::Result<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT count(*) FROM qa_pairs WHERE session_id = ?1",
            params![session_id],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asr::TranscriptRepo;
    use crate::storage::Db;

    fn session_id(db: &Db) -> String {
        TranscriptRepo::new(db.conn())
            .start_session("面试")
            .expect("session")
            .id
    }

    #[test]
    fn create_question_starts_with_empty_answer() {
        let db = Db::open_in_memory().expect("db");
        let session = session_id(&db);
        let repo = QaRepo::new(db.conn());

        let pair = repo
            .create_question(&session, "  介绍一下你的项目  ", 1_000)
            .expect("create");

        assert_eq!(pair.question, "介绍一下你的项目", "应去除首尾空白");
        assert!(pair.answer.is_empty());
        assert!(pair.sources.is_empty());
        assert_eq!(repo.count(&session).expect("count"), 1);
    }

    #[test]
    fn latest_returns_most_recent_question() {
        let db = Db::open_in_memory().expect("db");
        let session = session_id(&db);
        let repo = QaRepo::new(db.conn());

        repo.create_question(&session, "第一个问题", 1_000).expect("q1");
        repo.create_question(&session, "第二个问题", 2_000).expect("q2");

        assert_eq!(
            repo.latest(&session).expect("latest").expect("some").question,
            "第二个问题"
        );
    }

    #[test]
    fn list_is_ordered_and_limited() {
        let db = Db::open_in_memory().expect("db");
        let session = session_id(&db);
        let repo = QaRepo::new(db.conn());

        for index in 0..4 {
            repo.create_question(&session, &format!("问题{index}"), index)
                .expect("create");
        }

        let listed = repo.list(&session, 3).expect("list");
        assert_eq!(listed.len(), 3);
        assert_eq!(listed[0].question, "问题0");
    }

    #[test]
    fn set_answer_persists_answer_and_sources() {
        let db = Db::open_in_memory().expect("db");
        let session = session_id(&db);
        let repo = QaRepo::new(db.conn());

        let pair = repo.create_question(&session, "问题", 1_000).expect("create");
        assert!(repo
            .set_answer(&pair.id, "这是回答", &["笔记A".into(), "笔记B".into()])
            .expect("set"));

        let stored = repo.get(&pair.id).expect("get").expect("some");
        assert_eq!(stored.answer, "这是回答");
        assert_eq!(stored.sources, vec!["笔记A".to_string(), "笔记B".to_string()]);
    }

    #[test]
    fn set_answer_unknown_id_returns_false() {
        let db = Db::open_in_memory().expect("db");
        let repo = QaRepo::new(db.conn());
        assert!(!repo.set_answer("missing", "答案", &[]).expect("set"));
    }

    #[test]
    fn questions_are_scoped_by_session() {
        let db = Db::open_in_memory().expect("db");
        let first = session_id(&db);
        let second = session_id(&db);
        let repo = QaRepo::new(db.conn());

        repo.create_question(&first, "会话一的问题", 1).expect("q1");
        repo.create_question(&second, "会话二的问题", 2).expect("q2");

        assert_eq!(repo.count(&first).expect("count"), 1);
        assert_eq!(repo.list(&second, 10).expect("list")[0].question, "会话二的问题");
    }

    #[test]
    fn create_rejects_unknown_session() {
        let db = Db::open_in_memory().expect("db");
        let repo = QaRepo::new(db.conn());
        assert!(repo.create_question("missing", "问题", 1).is_err());
    }
}
