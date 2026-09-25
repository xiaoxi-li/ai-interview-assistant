//! 向量索引：分块嵌入写入 sqlite-vec，并提供语义召回。

use super::chunker::{self, DEFAULT_CHUNK_CHARS, DEFAULT_CHUNK_OVERLAP};
use super::SearchHit;
use crate::knowledge::store::{new_id, now_ms};
use crate::knowledge::NoteDto;
use crate::providers::types::{ProviderError, ProviderResult};
use crate::providers::EmbeddingProvider;
use crate::storage::{vec_to_blob, EMBEDDING_DIM};
use rusqlite::{params, Connection, OptionalExtension};

/// 重建单篇笔记的分块与向量（先删后插）
pub fn reindex_note(
    conn: &Connection,
    provider: &dyn EmbeddingProvider,
    note: &NoteDto,
) -> ProviderResult<usize> {
    if provider.dim() != EMBEDDING_DIM {
        return Err(ProviderError::InvalidResponse(format!(
            "嵌入维度不匹配：索引要求 {EMBEDDING_DIM}，实际 {}",
            provider.dim()
        )));
    }

    remove_note(conn, &note.id)?;

    // 标题参与索引，提升标题相关问题的召回
    let source = format!("{}\n\n{}", note.title, note.content);
    let pieces = chunker::chunk_text(&source, DEFAULT_CHUNK_CHARS, DEFAULT_CHUNK_OVERLAP);
    if pieces.is_empty() {
        return Ok(0);
    }

    let embeddings = provider.embed(&pieces)?;
    if embeddings.len() != pieces.len() {
        return Err(ProviderError::InvalidResponse(
            "嵌入结果数量与分块数量不一致".into(),
        ));
    }

    let tx = conn.unchecked_transaction()?;
    let now = now_ms();
    for (position, (text, vector)) in pieces.iter().zip(embeddings.iter()).enumerate() {
        let chunk_id = new_id("c");
        tx.execute(
            "INSERT INTO chunks (id, note_id, position, content, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![chunk_id, note.id, position as i64, text, now],
        )?;
        tx.execute(
            "INSERT INTO vec_chunks (chunk_id, embedding) VALUES (?1, ?2)",
            params![chunk_id, vec_to_blob(vector)],
        )?;
    }
    tx.commit()?;
    Ok(pieces.len())
}

/// 删除笔记的全部分块与向量
pub fn remove_note(conn: &Connection, note_id: &str) -> ProviderResult<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "DELETE FROM vec_chunks WHERE chunk_id IN (SELECT id FROM chunks WHERE note_id = ?1)",
        params![note_id],
    )?;
    tx.execute("DELETE FROM chunks WHERE note_id = ?1", params![note_id])?;
    tx.commit()?;
    Ok(())
}

/// 语义检索：返回按距离升序的笔记级命中（同一笔记取最相近的分块）
pub fn search_notes(
    conn: &Connection,
    provider: &dyn EmbeddingProvider,
    query: &str,
    top_k: usize,
) -> ProviderResult<Vec<SearchHit>> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let embeddings = provider.embed(&[query.to_string()])?;
    let Some(query_vector) = embeddings.into_iter().next() else {
        return Ok(Vec::new());
    };

    // 注意：sqlite-vec 要求 LIMIT 直接作用于 vec0 表的 KNN 查询，
    // 因此先在子查询中完成近邻召回，再与业务表关联。
    // 过度召回分块（top_k * 3），随后按笔记聚合去重。
    let mut stmt = conn.prepare(
        "SELECT c.note_id, c.content, knn.distance
         FROM (
             SELECT chunk_id, distance FROM vec_chunks
             WHERE embedding MATCH ?1 ORDER BY distance LIMIT ?2
         ) AS knn
         JOIN chunks c ON c.id = knn.chunk_id
         JOIN notes n ON n.id = c.note_id AND n.is_deleted = 0
         ORDER BY knn.distance",
    )?;
    let rows = stmt.query_map(params![vec_to_blob(&query_vector), (top_k * 3) as i64], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, f64>(2)?,
        ))
    })?;

    let mut hits: Vec<SearchHit> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for row in rows {
        let (note_id, chunk_content, distance) = row?;
        if seen.iter().any(|id| id == &note_id) {
            continue; // 同一笔记只保留最相近的分块
        }
        let title: Option<String> = conn
            .query_row(
                "SELECT title FROM notes WHERE id = ?1",
                params![note_id],
                |r| r.get(0),
            )
            .optional()?;
        let Some(title) = title else { continue };

        seen.push(note_id.clone());
        hits.push(SearchHit {
            note_id,
            title,
            snippet: chunk_content.chars().take(90).collect(),
            // 距离越小越相近，取负使其与其他检索结果"越大越相关"的语义一致
            score: -(distance as f32),
        });
        if hits.len() >= top_k {
            break;
        }
    }
    Ok(hits)
}

/// 清理孤儿向量（对应 chunk 已不存在的向量行）
pub fn prune_orphans(conn: &Connection) -> rusqlite::Result<usize> {
    let removed = conn.execute(
        "DELETE FROM vec_chunks WHERE chunk_id NOT IN (SELECT id FROM chunks)",
        [],
    )?;
    Ok(removed)
}

/// 统计已建立向量的分块数量
pub fn indexed_chunk_count(conn: &Connection) -> ProviderResult<usize> {
    let count: i64 = conn.query_row("SELECT count(*) FROM chunks", [], |r| r.get(0))?;
    Ok(count as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::store::NotesRepo;
    use crate::providers::MockEmbeddingProvider;
    use crate::storage::Db;

    fn setup() -> (Db, MockEmbeddingProvider) {
        (Db::open_in_memory().expect("db"), MockEmbeddingProvider::default())
    }

    #[test]
    fn reindex_creates_chunks_and_vectors() {
        let (db, provider) = setup();
        let note = NotesRepo::new(db.conn())
            .create("向量检索", "关于 语义 相似度 的 长 文本", None)
            .expect("create");

        let count = reindex_note(db.conn(), &provider, &note).expect("reindex");
        assert!(count >= 1);
        assert_eq!(indexed_chunk_count(db.conn()).expect("count"), count);
    }

    #[test]
    fn reindex_is_idempotent() {
        let (db, provider) = setup();
        let note = NotesRepo::new(db.conn())
            .create("标题", "内容", None)
            .expect("create");

        let first = reindex_note(db.conn(), &provider, &note).expect("first");
        let second = reindex_note(db.conn(), &provider, &note).expect("second");
        assert_eq!(first, second, "重复索引不应产生重复分块");
        assert_eq!(indexed_chunk_count(db.conn()).expect("count"), first);
    }

    #[test]
    fn semantic_search_finds_indexed_note() {
        let (db, provider) = setup();
        let repo = NotesRepo::new(db.conn());
        let note = repo
            .create("项目架构", "使用 Rust 构建桌面应用", None)
            .expect("create");
        reindex_note(db.conn(), &provider, &note).expect("reindex");

        let hits = search_notes(db.conn(), &provider, "Rust 桌面应用", 5).expect("search");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].note_id, note.id);
        assert!(!hits[0].snippet.is_empty());
    }

    #[test]
    fn search_on_empty_index_returns_nothing() {
        let (db, provider) = setup();
        let hits = search_notes(db.conn(), &provider, "任意查询", 5).expect("search");
        assert!(hits.is_empty());
    }

    #[test]
    fn empty_query_returns_nothing() {
        let (db, provider) = setup();
        assert!(search_notes(db.conn(), &provider, "   ", 5)
            .expect("search")
            .is_empty());
    }

    #[test]
    fn remove_note_clears_chunks_and_vectors() {
        let (db, provider) = setup();
        let note = NotesRepo::new(db.conn())
            .create("临时", "内容", None)
            .expect("create");
        reindex_note(db.conn(), &provider, &note).expect("reindex");

        remove_note(db.conn(), &note.id).expect("remove");
        assert_eq!(indexed_chunk_count(db.conn()).expect("count"), 0);
        assert!(search_notes(db.conn(), &provider, "内容", 5)
            .expect("search")
            .is_empty());
    }

    #[test]
    fn deleted_note_is_excluded_from_semantic_search() {
        let (db, provider) = setup();
        let repo = NotesRepo::new(db.conn());
        let note = repo.create("待删除", "语义 内容", None).expect("create");
        reindex_note(db.conn(), &provider, &note).expect("reindex");

        repo.trash(&note.id).expect("trash");
        assert!(search_notes(db.conn(), &provider, "语义 内容", 5)
            .expect("search")
            .is_empty());
    }

    #[test]
    fn dimension_mismatch_is_rejected() {
        let db = Db::open_in_memory().expect("db");
        let note = NotesRepo::new(db.conn())
            .create("标题", "内容", None)
            .expect("create");
        // MockEmbeddingProvider 需与索引维度一致
        let provider = MockEmbeddingProvider::new(64);
        assert!(reindex_note(db.conn(), &provider, &note).is_err());
    }
}
