//! 检索层：全文检索（FTS5 + jieba 中文分词）。
//!
//! P0 仅提供 BM25 全文召回；向量召回与 RRF 融合在后续任务接入，
//! 接口保持稳定以免上层改动。

pub mod fts;
pub mod tokenizer;

use serde::{Deserialize, Serialize};

pub use fts::{make_snippet, prune_orphans, remove_note, search_notes, sync_note};

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
