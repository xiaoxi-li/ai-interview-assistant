//! 检索层：全文检索（FTS5 + jieba 中文分词）。
//!
//! P0 仅提供 BM25 全文召回；向量召回与 RRF 融合在后续任务接入，
//! 接口保持稳定以免上层改动。

pub mod chunker;
pub mod fts;
pub mod fusion;
pub mod index;
pub mod tokenizer;

use crate::providers::types::ProviderResult;
use crate::providers::EmbeddingProvider;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub use fts::{make_snippet, prune_orphans, remove_note, search_notes, sync_note};
pub use fusion::{rrf_fuse, RRF_K};

/// 检索命中项
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub note_id: String,
    pub title: String,
    /// 命中位置上下文摘要
    pub snippet: String,
    /// 相关度（越大越相关）
    pub score: f32,
}

/// 混合检索：BM25 全文召回 + 向量语义召回，经 RRF 融合排序
pub fn search_hybrid(
    conn: &Connection,
    provider: &dyn EmbeddingProvider,
    query: &str,
    top_k: usize,
) -> ProviderResult<Vec<SearchHit>> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let recall = top_k.max(1) * 2;

    let fts_hits = fts::search_notes(conn, query, recall)?;
    // 向量通道「尽力而为」：Embedding 不可用（模型缺失/服务异常）时退化为纯关键词检索，
    // 不让整条召回链路失败（演示模式与离线场景同样受益）。
    let vector_hits = match index::search_notes(conn, provider, query, recall) {
        Ok(hits) => hits,
        Err(err) => {
            tracing::warn!("向量检索不可用，已退化为关键词检索：{err}");
            Vec::new()
        }
    };

    let fts_ranking: Vec<String> = fts_hits.iter().map(|h| h.note_id.clone()).collect();
    let vector_ranking: Vec<String> = vector_hits.iter().map(|h| h.note_id.clone()).collect();
    let fused = rrf_fuse(&[fts_ranking, vector_ranking], RRF_K);

    // 摘要优先采用全文命中（包含关键词上下文）
    let mut details: HashMap<String, SearchHit> = HashMap::new();
    for hit in vector_hits.into_iter().chain(fts_hits.into_iter()) {
        details.entry(hit.note_id.clone()).or_insert(hit);
    }

    let mut results = Vec::new();
    for (note_id, score) in fused.into_iter().take(top_k) {
        if let Some(mut hit) = details.remove(&note_id) {
            hit.score = score;
            results.push(hit);
        }
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::store::NotesRepo;
    use crate::providers::MockEmbeddingProvider;
    use crate::storage::Db;

    #[test]
    fn hybrid_search_returns_indexed_notes() {
        let db = Db::open_in_memory().expect("db");
        let provider = MockEmbeddingProvider::default();
        let repo = NotesRepo::new(db.conn());

        let note = repo
            .create("项目架构设计", "使用 Rust 与前端框架构建桌面应用", None)
            .expect("create");
        index::reindex_note(db.conn(), &provider, &note).expect("reindex");

        let hits = search_hybrid(db.conn(), &provider, "架构", 5).expect("hybrid");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].note_id, note.id);
        assert!(hits[0].score > 0.0, "融合得分应为正");
    }

    #[test]
    fn hybrid_search_empty_query_returns_nothing() {
        let db = Db::open_in_memory().expect("db");
        let provider = MockEmbeddingProvider::default();
        assert!(search_hybrid(db.conn(), &provider, "  ", 5).expect("hybrid").is_empty());
    }

    #[test]
    fn hybrid_search_merges_both_channels_with_positive_scores() {
        let db = Db::open_in_memory().expect("db");
        let provider = MockEmbeddingProvider::default();
        let repo = NotesRepo::new(db.conn());

        let first = repo
            .create("架构设计要点", "架构 设计 要点 说明", None)
            .expect("create first");
        let second = repo
            .create("架构随笔", "架构 随笔", None)
            .expect("create second");
        index::reindex_note(db.conn(), &provider, &first).expect("reindex first");
        index::reindex_note(db.conn(), &provider, &second).expect("reindex second");

        let hits = search_hybrid(db.conn(), &provider, "架构 设计 要点", 5).expect("hybrid");
        assert!(hits.len() >= 2, "两路召回结果应被合并");

        let returned: Vec<String> = hits.iter().map(|h| h.note_id.clone()).collect();
        assert!(returned.contains(&first.id));
        assert!(returned.contains(&second.id));
        assert!(hits.iter().all(|h| h.score > 0.0));
        // 融合得分应降序
        assert!(hits.windows(2).all(|w| w[0].score >= w[1].score));
    }
}
