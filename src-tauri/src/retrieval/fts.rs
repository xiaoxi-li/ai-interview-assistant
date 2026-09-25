//! FTS5 全文检索：索引同步、查询与摘要生成。

use super::{tokenizer, SearchHit};
use rusqlite::{params, Connection, OptionalExtension, Result};

/// 同步单篇笔记到 FTS 索引（先删后插，保证幂等）
pub fn sync_note(
    conn: &Connection,
    note_id: &str,
    title: &str,
    content: &str,
    tags: &[String],
) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM fts_index WHERE note_id = ?1", params![note_id])?;
    tx.execute(
        "INSERT INTO fts_index (note_id, title, content, tags) VALUES (?1, ?2, ?3, ?4)",
        params![
            note_id,
            tokenizer::tokenize(title),
            tokenizer::tokenize(content),
            tokenizer::tokenize(&tags.join(" "))
        ],
    )?;
    tx.commit()?;
    Ok(())
}

/// 从索引中移除笔记
pub fn remove_note(conn: &Connection, note_id: &str) -> Result<()> {
    conn.execute("DELETE FROM fts_index WHERE note_id = ?1", params![note_id])?;
    Ok(())
}

/// 清理已不存在笔记的索引残留（笔记被彻底删除后调用）
pub fn prune_orphans(conn: &Connection) -> Result<usize> {
    let removed = conn.execute(
        "DELETE FROM fts_index WHERE note_id NOT IN (SELECT id FROM notes)",
        [],
    )?;
    Ok(removed)
}

/// 全文检索：返回按相关度排序的命中（更高的 score 表示更相关）
pub fn search_notes(conn: &Connection, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
    let terms: Vec<String> = tokenizer::tokens(query)
        .into_iter()
        .filter(|t| tokenizer::is_meaningful(t))
        .collect();
    if terms.is_empty() {
        return Ok(Vec::new());
    }

    // 词元之间使用 OR 提高召回率；双引号包裹避免 FTS5 语法注入
    let match_expr = terms
        .iter()
        .map(|t| format!("\"{}\"", t.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" OR ");

    let mut stmt = conn.prepare(
        "SELECT note_id, bm25(fts_index) AS rank FROM fts_index
         WHERE fts_index MATCH ?1 ORDER BY rank LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![match_expr, limit as i64], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
    })?;

    let mut hits = Vec::new();
    for row in rows {
        let (note_id, rank) = row?;
        // 仅返回仍存在且未删除的笔记（索引可能短暂滞后）
        let note = conn
            .query_row(
                "SELECT title, content FROM notes WHERE id = ?1 AND is_deleted = 0",
                params![note_id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()?;
        let Some((title, content)) = note else {
            continue;
        };
        hits.push(SearchHit {
            snippet: make_snippet(&content, &terms, 90),
            note_id,
            title,
            // bm25 越小越相关，取负使其变为"越大越相关"
            score: -(rank as f32),
        });
    }
    Ok(hits)
}

/// 生成命中摘要：定位首个命中词并截取上下文
pub fn make_snippet(content: &str, terms: &[String], max_chars: usize) -> String {
    let flat = content.replace(['\n', '\r'], " ");
    let lowered = flat.to_lowercase();

    let byte_pos = terms
        .iter()
        .filter_map(|t| lowered.find(&t.to_lowercase()))
        .min();

    let Some(pos) = byte_pos else {
        let text: String = flat.trim().chars().take(max_chars).collect();
        return if flat.trim().chars().count() > max_chars {
            format!("{text}…")
        } else {
            text
        };
    };

    let char_pos = flat[..pos].chars().count();
    let start = char_pos.saturating_sub(max_chars / 3);
    let total = flat.chars().count();

    let mut snippet: String = flat.chars().skip(start).take(max_chars).collect();
    if start > 0 {
        snippet.insert(0, '…');
    }
    if start + max_chars < total {
        snippet.push('…');
    }
    snippet.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::store::NotesRepo;
    use crate::storage::Db;

    fn with_db<T>(f: impl FnOnce(&Connection) -> T) -> T {
        let db = Db::open_in_memory().expect("db");
        f(db.conn())
    }

    #[test]
    fn create_note_is_searchable_in_chinese() {
        with_db(|conn| {
            NotesRepo::new(conn)
                .create("项目架构设计", "使用 Rust 与前端框架构建桌面应用", None)
                .expect("create");

            let hits = search_notes(conn, "架构", 10).expect("search");
            assert_eq!(hits.len(), 1, "中文分词后应能命中");
            assert_eq!(hits[0].title, "项目架构设计");
            assert!(hits[0].snippet.contains("架构") || hits[0].snippet.contains("框架"));
        });
    }

    #[test]
    fn unmatched_query_returns_empty() {
        with_db(|conn| {
            NotesRepo::new(conn)
                .create("项目架构", "正文", None)
                .expect("create");
            assert!(search_notes(conn, "量子计算", 10).expect("search").is_empty());
        });
    }

    #[test]
    fn update_reindexes_content() {
        with_db(|conn| {
            let repo = NotesRepo::new(conn);
            let note = repo.create("标题", "旧内容 关于 缓存", None).expect("create");
            assert_eq!(search_notes(conn, "缓存", 10).expect("search").len(), 1);

            repo.update(&note.id, "标题", "新内容 关于 索引").expect("update");
            assert!(
                search_notes(conn, "缓存", 10).expect("search").is_empty(),
                "旧词应不再命中"
            );
            assert_eq!(search_notes(conn, "索引", 10).expect("search").len(), 1);
        });
    }

    #[test]
    fn trashed_note_is_excluded_from_search() {
        with_db(|conn| {
            let repo = NotesRepo::new(conn);
            let note = repo.create("待删除", "关于 向量检索", None).expect("create");
            assert_eq!(search_notes(conn, "向量", 10).expect("search").len(), 1);

            repo.trash(&note.id).expect("trash");
            assert!(search_notes(conn, "向量", 10).expect("search").is_empty());

            repo.restore(&note.id).expect("restore");
            assert_eq!(search_notes(conn, "向量", 10).expect("search").len(), 1);
        });
    }

    #[test]
    fn purge_prunes_fts_rows() {
        with_db(|conn| {
            let repo = NotesRepo::new(conn);
            let note = repo.create("临时", "关于 索引", None).expect("create");
            repo.trash(&note.id).expect("trash");
            repo.purge(&note.id).expect("purge");

            let count: i64 = conn
                .query_row("SELECT count(*) FROM fts_index", [], |r| r.get(0))
                .expect("count");
            assert_eq!(count, 0, "彻底删除后应无索引残留");
        });
    }

    #[test]
    fn search_ranks_more_relevant_first() {
        with_db(|conn| {
            let repo = NotesRepo::new(conn);
            repo.create("架构", "架构 架构 架构 设计", None).expect("a");
            repo.create("其他", "这里只提到一次 架构", None).expect("b");

            let hits = search_notes(conn, "架构", 10).expect("search");
            assert_eq!(hits.len(), 2);
            assert!(hits[0].score >= hits[1].score, "相关度应降序");
        });
    }

    #[test]
    fn snippet_centers_on_matched_term() {
        let content = "前言".repeat(50) + "目标关键词" + &"后记".repeat(50);
        let snippet = make_snippet(&content, &["目标关键词".to_string()], 30);
        assert!(snippet.contains("目标关键词"), "摘要应包含命中词：{snippet}");
        assert!(snippet.starts_with('…'), "长文本应带省略号");
    }

    #[test]
    fn snippet_handles_missing_term() {
        let snippet = make_snippet("完全无关的内容", &["不存在".to_string()], 20);
        assert_eq!(snippet, "完全无关的内容");
    }
}
