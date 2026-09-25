//! Provider 抽象层：AI / ASR / Embedding / VectorStore。
//!
//! 所有外部依赖（大模型、语音识别、嵌入模型、向量库）均通过 trait 抽象，
//! 便于：
//! * 多厂商可插拔（DeepSeek/Qwen/GLM/ChatGPT、讯飞/阿里/腾讯、sherpa-onnx）；
//! * 单元测试使用 Mock 实现，无需真实网络与音频设备；
//! * 未来替换底层实现（如 sqlite-vec → LanceDB）而不影响上层逻辑。

pub mod ai;
pub mod asr;
pub mod embedding;
pub mod types;
pub mod vector;

pub use ai::{AiProvider, ChatStream, MockAiProvider};
pub use asr::{AsrProvider, MockAsrProvider};
pub use embedding::{EmbeddingProvider, MockEmbeddingProvider};
pub use types::*;
pub use vector::{SqliteVecStore, VectorStore};
