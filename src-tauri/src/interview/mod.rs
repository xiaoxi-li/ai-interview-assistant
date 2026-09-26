//! 面试辅助子系统：问题识别与问答记录。
//!
//! IA-06 只负责「从面试官转写中识别问题」（自动 + 手动触发）；
//! 知识库召回（IA-07）与 AI 回答生成（IA-08）在此基础上继续。

pub mod question;
pub mod store;

pub use question::{
    analyze, analyze_with, DetectedQuestion, QuestionDetector, QuestionScore,
    DEFAULT_MERGE_WINDOW_MS, DEFAULT_MIN_CHARS, DEFAULT_THRESHOLD,
};
pub use store::{QaPairDto, QaRepo};
