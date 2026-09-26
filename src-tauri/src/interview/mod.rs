//! 面试辅助子系统：问题识别与问答记录。
//!
//! IA-06 负责「从面试官转写中识别问题」（自动 + 手动触发）；
//! IA-07 负责「按问题召回知识库片段」；IA-08 在此之上生成回答。

pub mod question;
pub mod recall;
pub mod store;

pub use question::{
    analyze, analyze_with, DetectedQuestion, QuestionDetector, QuestionScore,
    DEFAULT_MERGE_WINDOW_MS, DEFAULT_MIN_CHARS, DEFAULT_THRESHOLD,
};
pub use recall::{
    build_queries, recall_for_question, RecallHit, RecallOutcome, DEFAULT_TOP_K,
};
pub use store::{QaPairDto, QaRepo};
