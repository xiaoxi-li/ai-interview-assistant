//! 面试问题 → 知识库召回（IA-07）。
//!
//! 面试问句往往口语化且含大量疑问词（「介绍一下你最近负责的项目」），直接拿整句做
//! 检索会被无意义词稀释。因此这里做**双查询召回**：
//! 1. 完整问句（保留语义，交给向量通道）；
//! 2. 过滤疑问词/语气词后的关键词查询（提升 BM25 命中）。
//!
//! 两路结果按笔记去重、分值累加后截断，产出可直接作为回答引用的召回片段（IA-08 消费）。

use crate::providers::types::ProviderResult;
use crate::providers::EmbeddingProvider;
use crate::retrieval::tokenizer;
use crate::retrieval::search_hybrid;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// 默认召回条数
pub const DEFAULT_TOP_K: usize = 3;
/// 关键词查询最多使用的词元数
pub const MAX_KEYWORDS: usize = 8;

/// 问句中的无意义词（疑问词 / 语气词 / 人称 / 泛化动词）
const STOPWORDS: &[&str] = &[
    "请", "请问", "帮我", "麻烦", "一下", "介绍", "说说", "讲讲", "谈谈", "聊聊", "描述", "举例",
    "你", "您", "我", "我们", "你们", "的", "了", "吗", "呢", "吧", "是", "有", "会", "能", "过",
    "什么", "怎么", "为什么", "为何", "如何", "哪一", "哪个", "哪些", "哪里", "谁", "多少", "几个",
    "多久", "是否", "有没有", "能不能", "能否", "可不可以", "是不是", "这个", "那个", "这些", "那些",
    "然后", "就是", "以及", "关于", "对于", "嗯", "哦", "好的", "并且", "而且", "还有",
];

/// 是否为无检索意义的词
fn is_stopword(token: &str) -> bool {
    let lower = token.to_lowercase();
    STOPWORDS.iter().any(|word| lower == word.to_lowercase())
}

/// 由问题构造检索查询列表（完整问句 + 关键词问句）
pub fn build_queries(question: &str) -> Vec<String> {
    let normalized = question.trim();
    if normalized.is_empty() {
        return Vec::new();
    }

    let mut queries = vec![normalized.to_string()];

    let keywords: Vec<String> = tokenizer::tokens(normalized)
        .into_iter()
        .filter(|token| tokenizer::is_meaningful(token))
        .filter(|token| !is_stopword(token))
        // 单字中文词区分度低，英文/数字单词保留
        .filter(|token| token.chars().count() >= 2 || token.is_ascii())
        .take(MAX_KEYWORDS)
        .collect();

    if !keywords.is_empty() {
        let joined = keywords.join(" ");
        if joined != normalized {
            queries.push(joined);
        }
    }

    queries
}

/// 召回片段（可直接作为回答引用来源）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecallHit {
    pub note_id: String,
    pub title: String,
    pub snippet: String,
    pub score: f32,
}

/// 一次召回的过程（查询列表）与结果
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecallOutcome {
    pub question: String,
    pub queries: Vec<String>,
    pub hits: Vec<RecallHit>,
}

/// 按问题召回知识库片段：多查询召回 → 按笔记去重（分值累加）→ 排序截断
pub fn recall_for_question(
    conn: &Connection,
    provider: &dyn EmbeddingProvider,
    question: &str,
    top_k: usize,
) -> ProviderResult<RecallOutcome> {
    let queries = build_queries(question);
    let top_k = top_k.max(1);

    let mut merged: Vec<RecallHit> = Vec::new();
    for query in &queries {
        for hit in search_hybrid(conn, provider, query, top_k)? {
            match merged
                .iter_mut()
                .find(|existing| existing.note_id == hit.note_id)
            {
                Some(existing) => {
                    // 多路命中说明相关性更强：分值累加，摘要取更完整的
                    existing.score += hit.score;
                    if hit.snippet.chars().count() > existing.snippet.chars().count() {
                        existing.snippet = hit.snippet;
                    }
                }
                None => merged.push(RecallHit {
                    note_id: hit.note_id,
                    title: hit.title,
                    snippet: hit.snippet,
                    score: hit.score,
                }),
            }
        }
    }

    merged.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    merged.truncate(top_k);

    Ok(RecallOutcome {
        question: question.trim().to_string(),
        queries,
        hits: merged,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::store::NotesRepo;
    use crate::providers::MockEmbeddingProvider;
    use crate::retrieval::index;
    use crate::storage::Db;

    #[test]
    fn build_queries_keeps_full_question_and_strips_stopwords() {
        let queries = build_queries("介绍一下你最近负责的项目");
        assert_eq!(queries[0], "介绍一下你最近负责的项目");

        let keywords = &queries[1];
        assert!(keywords.contains("项目"), "关键词查询应保留实词：{keywords}");
        assert!(!keywords.contains("介绍"), "疑问词应被过滤：{keywords}");
        assert!(!keywords.contains('你'), "人称代词应被过滤：{keywords}");
    }

    #[test]
    fn build_queries_handles_empty_input() {
        assert!(build_queries("").is_empty());
        assert!(build_queries("   ").is_empty());
    }

    #[test]
    fn build_queries_drops_keyword_query_when_identical() {
        // 问题本身已是纯关键词时不应产生重复查询
        let queries = build_queries("架构设计");
        assert!(!queries.iter().any(|q| q == "架构设计 "), "{queries:?}");
        assert!(queries.len() <= 2);
    }

    #[test]
    fn build_queries_keeps_english_terms() {
        let queries = build_queries("介绍一下 Rust 的异步模型");
        let keywords = &queries[1];
        assert!(keywords.to_lowercase().contains("rust"), "{keywords}");
    }

    #[test]
    fn recall_returns_hits_for_question() {
        let db = Db::open_in_memory().expect("db");
        let provider = MockEmbeddingProvider::default();
        let repo = NotesRepo::new(db.conn());

        let note = repo
            .create(
                "项目架构设计",
                "负责桌面应用的架构设计与性能优化",
                None,
            )
            .expect("create");
        index::reindex_note(db.conn(), &provider, &note).expect("reindex");

        let outcome = recall_for_question(db.conn(), &provider, "介绍一下你负责的项目架构", 3)
            .expect("recall");

        assert_eq!(outcome.question, "介绍一下你负责的项目架构");
        assert_eq!(outcome.queries.len(), 2);
        assert_eq!(outcome.hits.len(), 1);
        assert_eq!(outcome.hits[0].note_id, note.id);
        assert_eq!(outcome.hits[0].title, "项目架构设计");
        assert!(outcome.hits[0].score > 0.0);
    }

    #[test]
    fn recall_merges_same_note_from_multiple_queries() {
        let db = Db::open_in_memory().expect("db");
        let provider = MockEmbeddingProvider::default();
        let repo = NotesRepo::new(db.conn());

        let note = repo
            .create("缓存方案", "缓存 方案 缓存击穿 缓存雪崩", None)
            .expect("create");
        index::reindex_note(db.conn(), &provider, &note).expect("reindex");

        let outcome =
            recall_for_question(db.conn(), &provider, "怎么解决缓存击穿", 3).expect("recall");
        // 同一笔记不会被重复返回
        assert_eq!(outcome.hits.len(), 1);
        assert_eq!(outcome.hits[0].note_id, note.id);
    }

    #[test]
    fn recall_respects_top_k() {
        let db = Db::open_in_memory().expect("db");
        let provider = MockEmbeddingProvider::default();
        let repo = NotesRepo::new(db.conn());

        for index_no in 0..4 {
            let note = repo
                .create(
                    &format!("架构笔记{index_no}"),
                    "架构 设计 项目 方案",
                    None,
                )
                .expect("create");
            index::reindex_note(db.conn(), &provider, &note).expect("reindex");
        }

        let outcome =
            recall_for_question(db.conn(), &provider, "介绍架构设计", 2).expect("recall");
        assert_eq!(outcome.hits.len(), 2);
        assert!(outcome.hits.windows(2).all(|w| w[0].score >= w[1].score));
    }

    #[test]
    fn recall_empty_question_returns_no_hits() {
        let db = Db::open_in_memory().expect("db");
        let provider = MockEmbeddingProvider::default();
        let outcome = recall_for_question(db.conn(), &provider, "   ", 3).expect("recall");
        assert!(outcome.queries.is_empty());
        assert!(outcome.hits.is_empty());
    }

    #[test]
    fn recall_without_index_returns_empty() {
        let db = Db::open_in_memory().expect("db");
        let provider = MockEmbeddingProvider::default();
        let outcome =
            recall_for_question(db.conn(), &provider, "介绍一个不存在的主题", 3).expect("recall");
        assert!(outcome.hits.is_empty());
    }
}
