//! AI Provider：统一 OpenAI 兼容协议的流式对话抽象。

use super::types::{ChatDelta, ChatRequest, ConnectionTest, ProviderError, ProviderResult};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

/// 流式对话增量接收端
pub type ChatStream = Receiver<ChatDelta>;

/// AI 能力抽象（DeepSeek / Qwen / GLM / ChatGPT 共用）
pub trait AiProvider: Send + Sync {
    /// Provider 标识
    fn name(&self) -> &str;

    /// 测试凭证与连通性
    fn test_connection(&self) -> ProviderResult<ConnectionTest>;

    /// 发起流式对话，返回增量事件流
    fn stream_chat(&self, req: ChatRequest) -> ProviderResult<ChatStream>;
}

/// 离线开发与测试用的 Mock AI：按脚本逐 token 输出
pub struct MockAiProvider {
    /// 依次输出的 token 片段
    pub script: Vec<String>,
    /// 每个 token 的模拟延迟（毫秒）
    pub token_latency_ms: u64,
}

impl MockAiProvider {
    pub fn new(script: Vec<String>) -> Self {
        Self { script, token_latency_ms: 0 }
    }
}

impl Default for MockAiProvider {
    fn default() -> Self {
        Self::new(vec!["这是".into(), "一段".into(), "流式".into(), "回答。".into()])
    }
}

fn send_deltas(tx: &Sender<ChatDelta>, script: Vec<String>, latency_ms: u64) {
    if tx.send(ChatDelta::Start).is_err() {
        return;
    }
    for text in script {
        if latency_ms > 0 {
            thread::sleep(Duration::from_millis(latency_ms));
        }
        if tx.send(ChatDelta::Token { text }).is_err() {
            return;
        }
    }
    let _ = tx.send(ChatDelta::Done { finish_reason: "stop".into() });
}

impl AiProvider for MockAiProvider {
    fn name(&self) -> &str {
        "mock"
    }

    fn test_connection(&self) -> ProviderResult<ConnectionTest> {
        Ok(ConnectionTest {
            ok: true,
            latency_ms: self.token_latency_ms,
            message: "mock provider 可用".into(),
        })
    }

    fn stream_chat(&self, req: ChatRequest) -> ProviderResult<ChatStream> {
        if req.messages.is_empty() {
            return Err(ProviderError::Other("对话消息不能为空".into()));
        }
        let (tx, rx) = mpsc::channel();
        let script = self.script.clone();
        let latency = self.token_latency_ms;
        thread::spawn(move || send_deltas(&tx, script, latency));
        Ok(rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::types::{collect_text, ChatMessage};

    fn req() -> ChatRequest {
        ChatRequest {
            model: "mock-model".into(),
            messages: vec![ChatMessage::user("你好")],
            temperature: 0.7,
        }
    }

    #[test]
    fn mock_streams_script_in_order() {
        let provider = MockAiProvider::new(vec!["A".into(), "B".into(), "C".into()]);
        let rx = provider.stream_chat(req()).expect("stream");

        let deltas: Vec<ChatDelta> = rx.iter().collect();
        assert_eq!(deltas.first(), Some(&ChatDelta::Start));
        assert_eq!(deltas.last(), Some(&ChatDelta::Done { finish_reason: "stop".into() }));
        assert_eq!(collect_text(&deltas), "ABC");
    }

    #[test]
    fn mock_rejects_empty_messages() {
        let provider = MockAiProvider::default();
        let empty = ChatRequest { model: "m".into(), messages: vec![], temperature: 0.0 };
        assert!(provider.stream_chat(empty).is_err());
    }

    #[test]
    fn mock_connection_test_ok() {
        let provider = MockAiProvider::default();
        let result = provider.test_connection().expect("test connection");
        assert!(result.ok);
    }

    #[test]
    fn chat_stream_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ChatStream>();
        assert_send::<MockAiProvider>();
    }
}
