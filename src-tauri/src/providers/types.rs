//! Provider 层共享类型与错误定义。

use serde::{Deserialize, Serialize};

/// Provider 层统一错误
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("provider 未配置")]
    NotConfigured,
    #[error("网络错误：{0}")]
    Network(String),
    #[error("响应无效：{0}")]
    InvalidResponse(String),
    #[error("存储错误：{0}")]
    Storage(String),
    #[error("{0}")]
    Other(String),
}

impl From<rusqlite::Error> for ProviderError {
    fn from(err: rusqlite::Error) -> Self {
        ProviderError::Storage(err.to_string())
    }
}

pub type ProviderResult<T> = std::result::Result<T, ProviderError>;

/// 连接/凭证测试结果
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConnectionTest {
    pub ok: bool,
    pub latency_ms: u64,
    pub message: String,
}

// ==================== AI ====================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: Role::System, content: content.into() }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: Role::User, content: content.into() }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: Role::Assistant, content: content.into() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: f32,
}

/// 流式对话增量事件
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatDelta {
    /// 流开始
    Start,
    /// 增量 token
    Token { text: String },
    /// 流结束
    Done { finish_reason: String },
    /// 错误
    Error { message: String },
}

/// 拼接一段完整回答（忽略起止与错误事件）
pub fn collect_text(deltas: &[ChatDelta]) -> String {
    deltas
        .iter()
        .filter_map(|d| match d {
            ChatDelta::Token { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

// ==================== ASR ====================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Speaker {
    /// 我（麦克风通道）
    Me,
    /// 面试官（系统音频通道）
    Interviewer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AsrMode {
    Cloud,
    Local,
}

/// 16kHz 单声道 PCM 音频帧
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioChunk {
    pub data: Vec<u8>,
    pub sample_rate: u32,
    pub channels: u16,
    pub ts: i64,
}

/// 转写事件
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TranscriptEvent {
    /// 中间结果（会被后续覆盖）
    Partial { speaker: Speaker, text: String },
    /// 最终结果
    Final { speaker: Speaker, text: String },
    Error { message: String },
}

// ==================== 向量 ====================

/// 向量检索命中项
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorHit {
    pub chunk_id: String,
    pub distance: f32,
}
