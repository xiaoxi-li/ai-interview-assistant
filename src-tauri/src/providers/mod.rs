//! Provider 抽象层：AI / ASR / Embedding / VectorStore。
//!
//! 所有外部依赖（大模型、语音识别、嵌入模型、向量库）均通过 trait 抽象，
//! 便于：
//! * 多厂商可插拔（DeepSeek/Qwen/GLM/硅基流动/OpenAI、讯飞/阿里/腾讯、sherpa-onnx）；
//! * 单元测试使用 Mock 实现，无需真实网络与音频设备；
//! * 未来替换底层实现（如 sqlite-vec → LanceDB）而不影响上层逻辑。

pub mod ai;
pub mod asr;
pub mod embedding;
pub mod openai;
pub mod thinking;
pub mod types;
pub mod vector;

pub use ai::{AiProvider, ChatStream, MockAiProvider};
pub use openai::OpenAiCompatProvider;
pub use asr::{AsrProvider, MockAsrProvider};
pub use embedding::{EmbeddingProvider, MockEmbeddingProvider};

/// 本地嵌入模型在 `models::catalog()` 中的 id（始终可用，供状态查询引用）
pub const LOCAL_EMBEDDING_MODEL_ID: &str = "bge-m3";

/// TODO-07（KB-07）：本地 ONNX 嵌入推理（BAAI/bge-m3）
#[cfg(feature = "local-embed-native")]
pub mod onnx_embedding;
#[cfg(feature = "local-embed-native")]
pub use onnx_embedding::OnnxEmbeddingProvider;
pub use types::*;
pub use vector::{SqliteVecStore, VectorStore};
