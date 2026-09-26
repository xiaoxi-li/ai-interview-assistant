//! 面试会话记录：浏览、回放时间线与导出（IA-10）。
//!
//! 会话记录由两部分组成（均在 P0 已实时落库）：
//! * `transcript_segments`：带说话人标注的转写时间线；
//! * `qa_pairs`：识别出的问题、AI 回答与引用来源。
//!
//! 本模块把它们组合成「会话详情」，并提供 Markdown / JSON 两种导出（纯函数，便于单测）。

use crate::asr::store::{SessionDto, TranscriptSegmentDto};
use crate::asr::TranscriptRepo;
use crate::interview::store::{QaPairDto, QaRepo};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 单次导出的转写条数上限（防止超大导出）
pub const MAX_EXPORT_SEGMENTS: usize = 5_000;
/// 单次导出的问答条数上限
pub const MAX_EXPORT_QUESTIONS: usize = 500;

/// 会话摘要（列表用）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummaryDto {
    pub id: String,
    pub title: String,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    /// 最终转写条数
    pub segment_count: usize,
    /// 识别出的问题数
    pub question_count: usize,
}

/// 会话详情（回放用）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetailDto {
    pub session: SessionDto,
    pub segments: Vec<TranscriptSegmentDto>,
    pub qa: Vec<QaPairDto>,
}

/// 导出结果
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionExportSummary {
    pub path: String,
    pub format: String,
    pub segments: usize,
    pub questions: usize,
    pub bytes: usize,
}

/// 相对会话开始的偏移（`mm:ss`），用于回放时间线
pub fn format_offset(ms: i64) -> String {
    let total_seconds = ms.max(0) / 1000;
    format!("{:02}:{:02}", total_seconds / 60, total_seconds % 60)
}

/// 列出会话摘要（按开始时间倒序）
pub fn list_sessions(conn: &Connection, limit: usize) -> rusqlite::Result<Vec<SessionSummaryDto>> {
    // 同毫秒创建的会话用 rowid 兜底，保证顺序稳定
    let sql = "SELECT id, title, started_at, ended_at FROM sessions
               ORDER BY started_at DESC, rowid DESC LIMIT ?1";
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([limit.max(1) as i64], |row| {
        Ok(SessionDto {
            id: row.get(0)?,
            title: row.get(1)?,
            started_at: row.get(2)?,
            ended_at: row.get(3)?,
        })
    })?;

    let mut summaries = Vec::new();
    for row in rows {
        let session = row?;
        let transcript = TranscriptRepo::new(conn);
        summaries.push(SessionSummaryDto {
            segment_count: transcript.count_finals(&session.id)?,
            question_count: QaRepo::new(conn).count(&session.id)?,
            id: session.id,
            title: session.title,
            started_at: session.started_at,
            ended_at: session.ended_at,
        });
    }
    Ok(summaries)
}

/// 读取会话详情（不存在时返回 None）
pub fn session_detail(
    conn: &Connection,
    session_id: &str,
) -> rusqlite::Result<Option<SessionDetailDto>> {
    let transcript = TranscriptRepo::new(conn);
    let Some(session) = transcript.get_session(session_id)? else {
        return Ok(None);
    };

    let segments = transcript.list_finals(session_id)?;
    let qa = QaRepo::new(conn).list(session_id, MAX_EXPORT_QUESTIONS)?;

    Ok(Some(SessionDetailDto {
        session,
        segments,
        qa,
    }))
}

/// 删除会话（转写段落与问答记录由外键级联删除）
pub fn delete_session(conn: &Connection, session_id: &str) -> rusqlite::Result<bool> {
    let affected = conn.execute("DELETE FROM sessions WHERE id = ?1", [session_id])?;
    Ok(affected > 0)
}

/// 渲染为 Markdown（时间线 + 问答记录）
pub fn session_to_markdown(detail: &SessionDetailDto) -> String {
    let mut out = String::new();
    let start = detail.session.started_at;

    out.push_str(&format!("# 面试会话：{}\n\n", detail.session.title));
    out.push_str(&format!("- 会话 ID：`{}`\n", detail.session.id));
    out.push_str(&format!("- 开始时间（Unix 毫秒）：{}\n", start));
    match detail.session.ended_at {
        Some(ended) => out.push_str(&format!(
            "- 结束时间（Unix 毫秒）：{}（时长 {}）\n",
            ended,
            format_offset(ended - start)
        )),
        None => out.push_str("- 结束时间：（会话未正常结束）\n"),
    }
    out.push_str(&format!("- 转写条数：{}\n", detail.segments.len()));
    out.push_str(&format!("- 问答组数：{}\n\n", detail.qa.len()));

    out.push_str("## 转写时间线\n\n");
    if detail.segments.is_empty() {
        out.push_str("（无转写记录）\n\n");
    } else {
        for segment in detail.segments.iter().take(MAX_EXPORT_SEGMENTS) {
            let speaker = if segment.speaker == "me" {
                "我"
            } else {
                "面试官"
            };
            out.push_str(&format!(
                "- **{speaker}**（{}）：{}\n",
                format_offset(segment.ts - start),
                segment.text.trim()
            ));
        }
        out.push('\n');
    }

    out.push_str("## 问答记录\n\n");
    if detail.qa.is_empty() {
        out.push_str("（无问答记录）\n");
    } else {
        for (index, pair) in detail.qa.iter().take(MAX_EXPORT_QUESTIONS).enumerate() {
            out.push_str(&format!(
                "### Q{}. {}\n\n",
                index + 1,
                pair.question.trim()
            ));
            out.push_str(&format!(
                "**时间**：{}\n\n",
                format_offset(pair.ts - start)
            ));

            if pair.answer.trim().is_empty() {
                out.push_str("**回答**：（未生成）\n\n");
            } else {
                out.push_str(&format!("**回答**：\n\n{}\n\n", pair.answer.trim()));
            }

            if pair.sources.is_empty() {
                out.push_str("**引用来源**：（无）\n\n");
            } else {
                out.push_str("**引用来源**：\n\n");
                for (source_index, source) in pair.sources.iter().enumerate() {
                    let title = if source.title.trim().is_empty() {
                        "未命名笔记"
                    } else {
                        source.title.trim()
                    };
                    out.push_str(&format!("{}. {title}\n", source_index + 1));
                }
                out.push('\n');
            }
        }
    }

    out
}

/// 渲染为 JSON（结构化备份）
pub fn session_to_json(detail: &SessionDetailDto) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(detail)
}

/// 导出会话到文件；`format` 取 `markdown` / `json`
pub fn export_session_to_file(
    detail: &SessionDetailDto,
    dest: &Path,
    format: &str,
) -> Result<SessionExportSummary, String> {
    let (content, normalized) = match format.trim().to_ascii_lowercase().as_str() {
        "json" => (
            session_to_json(detail).map_err(|e| e.to_string())?,
            "json".to_string(),
        ),
        "markdown" | "md" => (session_to_markdown(detail), "markdown".to_string()),
        other => return Err(format!("不支持的导出格式：{other}")),
    };

    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("创建导出目录失败（{}）：{e}", parent.display()))?;
        }
    }

    std::fs::write(dest, content.as_bytes())
        .map_err(|e| format!("写入导出文件失败（{}）：{e}", dest.display()))?;

    Ok(SessionExportSummary {
        path: PathBuf::from(dest).to_string_lossy().to_string(),
        format: normalized,
        segments: detail.segments.len(),
        questions: detail.qa.len(),
        bytes: content.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interview::store::SourceRef;
    use crate::providers::types::Speaker;
    use crate::storage::Db;

    fn temp_dir(tag: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "aia-session-{tag}-{}-{}",
            std::process::id(),
            crate::knowledge::store::now_ms()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    /// 构造一场含转写与问答的会话
    fn seed(conn: &Connection) -> String {
        let transcript = TranscriptRepo::new(conn);
        let session = transcript.start_session("技术面 · 后端").expect("session");

        transcript
            .append(
                &session.id,
                Speaker::Interviewer,
                "介绍一下你最近负责的项目",
                true,
                session.started_at + 12_000,
            )
            .expect("segment 1");
        transcript
            .append(
                &session.id,
                Speaker::Me,
                "我负责桌面端架构设计",
                true,
                session.started_at + 20_000,
            )
            .expect("segment 2");

        let qa = QaRepo::new(conn);
        let pair = qa
            .create_question(&session.id, "介绍一下你最近负责的项目", session.started_at + 12_000)
            .expect("question");
        qa.set_answer(
            &pair.id,
            "我负责桌面端应用架构设计。",
            &[SourceRef {
                note_id: "n-1".into(),
                title: "项目架构设计".into(),
            }],
        )
        .expect("answer");

        session.id
    }

    #[test]
    fn list_sessions_reports_counts_and_order() {
        let db = Db::open_in_memory().expect("db");
        let first = seed(db.conn());
        let second = TranscriptRepo::new(db.conn())
            .start_session("HR 面")
            .expect("second");

        let sessions = list_sessions(db.conn(), 10).expect("list");
        assert_eq!(sessions.len(), 2);
        // 后创建的会话开始时间更晚，应排在最前
        assert_eq!(sessions[0].id, second.id);
        assert_eq!(sessions[0].segment_count, 0);
        assert_eq!(sessions[1].id, first);
        assert_eq!(sessions[1].segment_count, 2);
        assert_eq!(sessions[1].question_count, 1);
    }

    #[test]
    fn list_sessions_respects_limit() {
        let db = Db::open_in_memory().expect("db");
        for index in 0..3 {
            TranscriptRepo::new(db.conn())
                .start_session(&format!("会话{index}"))
                .expect("session");
        }
        assert_eq!(list_sessions(db.conn(), 2).expect("list").len(), 2);
    }

    #[test]
    fn session_detail_includes_transcript_and_qa() {
        let db = Db::open_in_memory().expect("db");
        let id = seed(db.conn());

        let detail = session_detail(db.conn(), &id).expect("detail").expect("exists");
        assert_eq!(detail.session.title, "技术面 · 后端");
        assert_eq!(detail.segments.len(), 2);
        assert_eq!(detail.qa.len(), 1);
        assert_eq!(detail.qa[0].sources[0].title, "项目架构设计");
    }

    #[test]
    fn session_detail_missing_session_is_none() {
        let db = Db::open_in_memory().expect("db");
        assert!(session_detail(db.conn(), "missing").expect("detail").is_none());
    }

    #[test]
    fn markdown_contains_timeline_qa_and_sources() {
        let db = Db::open_in_memory().expect("db");
        let id = seed(db.conn());
        let detail = session_detail(db.conn(), &id)
            .expect("detail")
            .expect("exists");

        let markdown = session_to_markdown(&detail);
        assert!(markdown.contains("# 面试会话：技术面 · 后端"));
        assert!(markdown.contains("## 转写时间线"));
        assert!(markdown.contains("**面试官**（00:12）：介绍一下你最近负责的项目"));
        assert!(markdown.contains("**我**（00:20）：我负责桌面端架构设计"));
        assert!(markdown.contains("### Q1. 介绍一下你最近负责的项目"));
        assert!(markdown.contains("我负责桌面端应用架构设计。"));
        assert!(markdown.contains("1. 项目架构设计"));
    }

    #[test]
    fn markdown_handles_empty_session() {
        let db = Db::open_in_memory().expect("db");
        let id = TranscriptRepo::new(db.conn())
            .start_session("空会话")
            .expect("session")
            .id;
        let detail = session_detail(db.conn(), &id)
            .expect("detail")
            .expect("exists");

        let markdown = session_to_markdown(&detail);
        assert!(markdown.contains("（无转写记录）"));
        assert!(markdown.contains("（无问答记录）"));
        assert!(markdown.contains("（会话未正常结束）"));
    }

    #[test]
    fn markdown_marks_missing_answer_and_sources() {
        let db = Db::open_in_memory().expect("db");
        let transcript = TranscriptRepo::new(db.conn());
        let id = transcript.start_session("待回答").expect("session").id;
        QaRepo::new(db.conn())
            .create_question(&id, "为什么选择 Rust", 5_000)
            .expect("question");

        let detail = session_detail(db.conn(), &id)
            .expect("detail")
            .expect("exists");
        let markdown = session_to_markdown(&detail);
        assert!(markdown.contains("**回答**：（未生成）"));
        assert!(markdown.contains("**引用来源**：（无）"));
    }

    #[test]
    fn offset_formatting_is_mm_ss() {
        assert_eq!(format_offset(0), "00:00");
        assert_eq!(format_offset(12_000), "00:12");
        assert_eq!(format_offset(605_000), "10:05");
        assert_eq!(format_offset(-5), "00:00");
    }

    #[test]
    fn export_writes_markdown_and_json_files() {
        let db = Db::open_in_memory().expect("db");
        let id = seed(db.conn());
        let detail = session_detail(db.conn(), &id)
            .expect("detail")
            .expect("exists");
        let dir = temp_dir("export");

        let md_path = dir.join("session.md");
        let summary = export_session_to_file(&detail, &md_path, "markdown").expect("export md");
        assert_eq!(summary.format, "markdown");
        assert_eq!(summary.segments, 2);
        assert_eq!(summary.questions, 1);
        assert!(summary.bytes > 0);
        let written = std::fs::read_to_string(&md_path).expect("read md");
        assert!(written.contains("# 面试会话"));

        let json_path = dir.join("nested/session.json");
        let summary = export_session_to_file(&detail, &json_path, "json").expect("export json");
        assert_eq!(summary.format, "json");
        let json_text = std::fs::read_to_string(&json_path).expect("read json");
        assert!(json_text.contains("\"session\""));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn export_rejects_unknown_format() {
        let db = Db::open_in_memory().expect("db");
        let id = seed(db.conn());
        let detail = session_detail(db.conn(), &id)
            .expect("detail")
            .expect("exists");
        let dir = temp_dir("bad-format");

        let error = export_session_to_file(&detail, &dir.join("x.txt"), "pdf").expect_err("应失败");
        assert!(error.contains("不支持的导出格式"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn delete_session_cascades_records() {
        let db = Db::open_in_memory().expect("db");
        let id = seed(db.conn());

        assert!(delete_session(db.conn(), &id).expect("delete"));
        assert!(session_detail(db.conn(), &id).expect("detail").is_none());
        assert_eq!(
            TranscriptRepo::new(db.conn()).count_segments(&id).expect("count"),
            0
        );
        assert_eq!(QaRepo::new(db.conn()).count(&id).expect("count qa"), 0);
        assert!(!delete_session(db.conn(), &id).expect("delete twice"));
    }
}
