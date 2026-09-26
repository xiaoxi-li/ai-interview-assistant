//! 问答持久化：识别出的问题与 AI 回答（`qa_pairs`）。

use crate::knowledge::store::new_id;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

/// 引用来源（IA-09）：回答引用了哪篇笔记，可据此跳转定位
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRef {
    pub note_id: String,
    pub title: String,
}

/// 解析引用来源 JSON。
///
/// 兼容早期版本只存标题数组（`["笔记A"]`）的格式，避免历史会话回放时报错。
fn parse_sources(json: &str) -> Vec<SourceRef> {
    if let Ok(refs) = serde_json::from_str::<Vec<SourceRef>>(json) {
        return refs;
    }
    if let Ok(titles) = serde_json::from_str::<Vec<String>>(json) {
        return titles
            .into_iter()
            .map(|title| SourceRef {
                note_id: String::new(),
                title,
            })
            .collect();
    }
    Vec::new()
}

/// 问答对
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QaPairDto {
    pub id: String,
    pub session_id: String,
    pub question: String,
    pub answer: String,
    /// 引用来源（笔记 ID + 标题）
    pub sources: Vec<SourceRef>,
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
        sources: parse_sources(&sources_json),
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
        sources: &[SourceRef],
    ) -> rusqlite::Result<bool> {
        let sources_json = serde_json::to_string(sources).unwrap_or_else(|_| "[]".to_string());
        let affected = self.conn.execute(
            "UPDATE qa_pairs SET answer = ?2, sources_json = ?3 WHERE id = ?1",
            params![id, answer, sources_json],
        )?;
        Ok(affected > 0)
    }

    /// 仅更新引用来源（IA-07 召回结果落库，回答留待 IA-08 生成）
    pub fn set_sources(&self, id: &str, sources: &[SourceRef]) -> rusqlite::Result<bool> {
        let sources_json = serde_json::to_string(sources).unwrap_or_else(|_| "[]".to_string());
        let affected = self.conn.execute(
            "UPDATE qa_pairs SET sources_json = ?2 WHERE id = ?1",
            params![id, sources_json],
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

    fn source(note_id: &str, title: &str) -> SourceRef {
        SourceRef {
            note_id: note_id.to_string(),
            title: title.to_string(),
        }
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
            .set_answer(
                &pair.id,
                "这是回答",
                &[source("n-a", "笔记A"), source("n-b", "笔记B")]
            )
            .expect("set"));

        let stored = repo.get(&pair.id).expect("get").expect("some");
        assert_eq!(stored.answer, "这是回答");
        assert_eq!(
            stored.sources,
            vec![source("n-a", "笔记A"), source("n-b", "笔记B")]
        );
    }

    #[test]
    fn set_sources_only_touches_sources() {
        let db = Db::open_in_memory().expect("db");
        let session = session_id(&db);
        let repo = QaRepo::new(db.conn());

        let pair = repo.create_question(&session, "问题", 1_000).expect("create");
        repo.set_answer(&pair.id, "已有回答", &[]).expect("answer");
        assert!(repo
            .set_sources(&pair.id, &[source("n-1", "笔记X")])
            .expect("set sources"));

        let stored = repo.get(&pair.id).expect("get").expect("some");
        assert_eq!(stored.sources, vec![source("n-1", "笔记X")]);
        assert_eq!(stored.answer, "已有回答", "更新来源不应清空回答");
    }

    #[test]
    fn legacy_string_sources_are_still_readable() {
        // 早期版本 sources_json 只存标题数组
        let parsed = parse_sources(r#"["笔记A","笔记B"]"#);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].title, "笔记A");
        assert!(parsed[0].note_id.is_empty(), "旧格式无笔记 ID");

        // 新格式（对象数组）优先
        let parsed = parse_sources(r#"[{"noteId":"n-1","title":"笔记A"}]"#);
        assert_eq!(parsed[0].note_id, "n-1");

        // 损坏数据安全降级
        assert!(parse_sources("{ broken").is_empty());
    }

    #[test]
    fn set_sources_unknown_id_returns_false() {
        let db = Db::open_in_memory().expect("db");
        let repo = QaRepo::new(db.conn());
        assert!(!repo.set_sources("missing", &[]).expect("set"));
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
