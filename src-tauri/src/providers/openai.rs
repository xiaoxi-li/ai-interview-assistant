//! OpenAI 兼容 Provider：覆盖 DeepSeek / Qwen / GLM / 硅基流动 / OpenAI。
//!
//! * 连接测试走轻量的 `GET /models`，失败时回退提示；
//! * 流式对话走 `POST /chat/completions`（`stream: true`），按 SSE 解析增量；
//! * 思考 / 非思考模式（ST-12）由 [`super::thinking`] 按平台下发，上游拒绝该参数时去参重试一次。

use super::ai::{AiProvider, ChatStream};
use super::thinking;
use super::types::{
    ChatDelta, ChatMessage, ChatRequest, ConnectionTest, ProviderError, ProviderResult, Role,
};
use crate::config::AiConfig;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// 连接测试超时
const TEST_TIMEOUT: Duration = Duration::from_secs(15);
/// 流式对话整体超时
const STREAM_TIMEOUT: Duration = Duration::from_secs(180);

pub struct OpenAiCompatProvider {
    config: AiConfig,
    api_key: Option<String>,
    agent: ureq::Agent,
}

impl OpenAiCompatProvider {
    pub fn new(config: AiConfig, api_key: Option<String>) -> Self {
        Self {
            config,
            api_key,
            agent: ureq::AgentBuilder::new().build(),
        }
    }

    fn base(&self) -> String {
        self.config.base_url.trim_end_matches('/').to_string()
    }

    /// 模型列表地址（连接测试）
    pub fn models_url(&self) -> String {
        format!("{}/models", self.base())
    }

    /// 对话补全地址（流式）
    pub fn chat_url(&self) -> String {
        format!("{}/chat/completions", self.base())
    }

    fn apply_auth(&self, request: ureq::Request) -> ureq::Request {
        match self.api_key.as_deref() {
            Some(key) if !key.is_empty() => request.set("Authorization", &format!("Bearer {key}")),
            _ => request,
        }
    }

    /// 发送流式对话请求（思考参数的下发与回退由调用方负责）
    fn send_chat(&self, body: &Value) -> ProviderResult<ureq::Response> {
        self.apply_auth(self.agent.post(&self.chat_url()))
            .set("Content-Type", "application/json")
            .set("Accept", "text/event-stream")
            .timeout(STREAM_TIMEOUT)
            .send_json(body)
            .map_err(http_error)
    }
}

/// 把 ureq 错误转换为 `ProviderError`，尽量保留服务端返回的错误详情
fn http_error(err: ureq::Error) -> ProviderError {
    match err {
        ureq::Error::Status(code, response) => {
            let detail = response
                .into_json::<Value>()
                .ok()
                .and_then(|value| {
                    value
                        .get("error")
                        .and_then(|error| error.get("message"))
                        .and_then(Value::as_str)
                        .or_else(|| value.get("message").and_then(Value::as_str))
                        .map(str::to_string)
                })
                .unwrap_or_default();
            if detail.trim().is_empty() {
                ProviderError::Network(format!("服务返回 HTTP {code}"))
            } else {
                ProviderError::Network(format!("服务返回 HTTP {code}：{detail}"))
            }
        }
        other => ProviderError::Network(other.to_string()),
    }
}

/// 判断错误是否由「思考模式参数不被模型支持」引起（决定是否去参重试）
fn is_thinking_param_error(err: &ProviderError) -> bool {
    match err {
        ProviderError::Network(message) => thinking::is_param_rejected(message),
        _ => false,
    }
}

/// 将内部消息转换为 OpenAI 协议格式
pub fn build_messages(messages: &[ChatMessage]) -> Vec<Value> {
    messages
        .iter()
        .map(|message| {
            let role = match message.role {
                Role::System => "system",
                Role::User => "user",
                Role::Assistant => "assistant",
            };
            json!({ "role": role, "content": message.content })
        })
        .collect()
}

/// 构造流式请求体；请求未指定模型时回退到配置中的默认模型
pub fn build_chat_body(config: &AiConfig, request: &ChatRequest) -> Value {
    let model = if request.model.trim().is_empty() {
        config.model.clone()
    } else {
        request.model.clone()
    };

    json!({
        "model": model,
        "messages": build_messages(&request.messages),
        "temperature": request.temperature,
        "stream": true,
    })
}

/// 解析 SSE 数据行：`data: {...}` / `data: [DONE]` / 其余忽略
pub fn parse_sse_line(line: &str) -> Option<ChatDelta> {
    let payload = line.strip_prefix("data:")?.trim();
    if payload.is_empty() {
        return None;
    }
    if payload == "[DONE]" {
        return Some(ChatDelta::Done {
            finish_reason: "stop".into(),
        });
    }

    let value: Value = serde_json::from_str(payload).ok()?;
    // 流中可能带错误对象
    if let Some(error) = value.get("error") {
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("未知错误");
        return Some(ChatDelta::Error {
            message: message.to_string(),
        });
    }

    let choices = value.get("choices")?.as_array()?;
    let choice = choices.first()?;
    let finish = choice.get("finish_reason").and_then(Value::as_str);

    if let Some(reason) = finish {
        if !reason.is_empty() && reason != "null" {
            return Some(ChatDelta::Done {
                finish_reason: reason.to_string(),
            });
        }
    }

    let text = choice
        .get("delta")
        .and_then(|d| d.get("content"))
        .and_then(Value::as_str)
        .unwrap_or_default();

    if text.is_empty() {
        None
    } else {
        Some(ChatDelta::Token {
            text: text.to_string(),
        })
    }
}

impl AiProvider for OpenAiCompatProvider {
    fn name(&self) -> &str {
        self.config.platform.as_str()
    }

    fn test_connection(&self) -> ProviderResult<ConnectionTest> {
        let started = Instant::now();
        let request = self
            .apply_auth(self.agent.get(&self.models_url()))
            .timeout(TEST_TIMEOUT);

        let result = request.call();
        let latency_ms = started.elapsed().as_millis() as u64;

        match result {
            Ok(response) => Ok(ConnectionTest {
                ok: true,
                latency_ms,
                message: format!("连接成功（HTTP {}）", response.status()),
            }),
            Err(ureq::Error::Status(code, response)) => {
                let detail = response
                    .into_json::<Value>()
                    .ok()
                    .and_then(|v| {
                        v.get("error")
                            .and_then(|e| e.get("message"))
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    })
                    .unwrap_or_else(|| format!("HTTP {code}"));
                Ok(ConnectionTest {
                    ok: false,
                    latency_ms,
                    message: format!("服务返回错误：{detail}"),
                })
            }
            Err(err) => Err(ProviderError::Network(err.to_string())),
        }
    }

    fn stream_chat(&self, request: ChatRequest) -> ProviderResult<ChatStream> {
        let mut body = build_chat_body(&self.config, &request);
        let thinking_keys = thinking::apply(&mut body, self.config.platform, self.config.thinking);

        let response = match self.send_chat(&body) {
            Ok(response) => response,
            // 模型/网关不支持该思考参数（例如给 qwen-plus 传 enable_thinking）：
            // 去掉参数重试一次，避免「模式开关」把回答功能整体带崩。
            Err(err) if !thinking_keys.is_empty() && is_thinking_param_error(&err) => {
                tracing::warn!(
                    "上游拒绝思考模式参数，已去掉参数重试（platform={}）：{err}",
                    self.config.platform.as_str()
                );
                thinking::remove(&mut body, &thinking_keys);
                self.send_chat(&body)?
            }
            Err(err) => return Err(err),
        };

        let (tx, rx) = mpsc::channel();
        let reader = BufReader::new(response.into_reader());

        if tx.send(ChatDelta::Start).is_err() {
            return Ok(rx);
        }

        thread::spawn(move || {
            for line in reader.lines() {
                let Ok(line) = line else { break };
                let Some(delta) = parse_sse_line(line.trim()) else {
                    continue;
                };
                let is_done = matches!(delta, ChatDelta::Done { .. } | ChatDelta::Error { .. });
                if tx.send(delta).is_err() {
                    return;
                }
                if is_done {
                    return;
                }
            }
            // 上游未发送 [DONE] 时补齐结束事件
            let _ = tx.send(ChatDelta::Done {
                finish_reason: "stop".into(),
            });
        });

        Ok(rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use crate::config::{AiPlatform, ThinkingMode};

    fn config_with(base_url: String) -> AiConfig {
        config_for(AiPlatform::Deepseek, ThinkingMode::NonThinking, base_url)
    }

    /// 指定平台与思考模式的配置（思考参数相关用例）
    fn config_for(platform: AiPlatform, thinking: ThinkingMode, base_url: String) -> AiConfig {
        AiConfig {
            platform,
            base_url,
            model: "test-model".into(),
            stream: true,
            temperature: 0.5,
            thinking,
        }
    }

    fn request() -> ChatRequest {
        ChatRequest {
            model: "test-model".into(),
            messages: vec![ChatMessage::user("你好")],
            temperature: 0.5,
        }
    }

    /// 查找 HTTP 头部结束位置（\r\n\r\n）
    fn header_end(buf: &[u8]) -> Option<usize> {
        buf.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4)
    }

    /// 解析 Content-Length
    fn content_length(head: &str) -> usize {
        head.lines()
            .find(|line| line.to_ascii_lowercase().starts_with("content-length:"))
            .and_then(|line| line.split(':').nth(1))
            .and_then(|value| value.trim().parse().ok())
            .unwrap_or(0)
    }

    /// 完整读取一个 HTTP 请求（头部 + Content-Length 指定的 body）。
    /// 若只读头部就响应并关闭连接，客户端在写 body 时会遇到连接重置。
    fn read_full_request(stream: &mut std::net::TcpStream) -> String {
        let mut buffer: Vec<u8> = Vec::new();
        let mut chunk = [0u8; 1024];

        loop {
            let read = stream.read(&mut chunk).unwrap_or(0);
            if read == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..read]);

            if let Some(end) = header_end(&buffer) {
                let head = String::from_utf8_lossy(&buffer[..end]).to_string();
                let mut remaining = content_length(&head).saturating_sub(buffer.len() - end);
                while remaining > 0 {
                    let read = stream.read(&mut chunk).unwrap_or(0);
                    if read == 0 {
                        break;
                    }
                    buffer.extend_from_slice(&chunk[..read]);
                    remaining = remaining.saturating_sub(read);
                }
                break;
            }
        }
        String::from_utf8_lossy(&buffer).to_string()
    }

    /// 启动一个极简 HTTP 服务：完整接收 1 个请求后返回固定响应并退出。
    /// 注意：处理请求数必须与调用方实际请求数一致，否则线程会阻塞在 accept 上导致测试挂起。
    fn spawn_server(handler: fn(&str) -> String) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let handle = thread::spawn(move || {
            for stream in listener.incoming().take(1) {
                let Ok(mut stream) = stream else { continue };
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                let request_text = read_full_request(&mut stream);
                let response = handler(&request_text);
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });
        (format!("http://{addr}"), handle)
    }

    /// 启动一个可返回多轮响应的极简 HTTP 服务，并回传收到的全部请求明文。
    /// 用非阻塞 accept + 截止时间，避免调用方少发请求时线程一直阻塞在 accept 上。
    fn spawn_sequence_server(responses: Vec<String>) -> (String, thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        listener.set_nonblocking(true).expect("nonblocking");
        let expected = responses.len();

        let handle = thread::spawn(move || {
            let mut requests: Vec<String> = Vec::new();
            let deadline = Instant::now() + Duration::from_secs(5);

            while requests.len() < expected && Instant::now() < deadline {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_nonblocking(false);
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                        let request = read_full_request(&mut stream);
                        let index = requests.len();
                        requests.push(request);
                        if let Some(response) = responses.get(index) {
                            let _ = stream.write_all(response.as_bytes());
                            let _ = stream.flush();
                        }
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
            requests
        });

        (format!("http://{addr}"), handle)
    }

    fn error_response(code: u16, message: &str) -> String {
        let body = serde_json::json!({ "error": { "message": message } }).to_string();
        format!(
            "HTTP/1.1 {code} Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    fn models_response(_req: &str) -> String {
        let body = r#"{"data":[{"id":"test-model"}]}"#;
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    fn sse_response(_req: &str) -> String {
        let body = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"你好\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"，世界\"}}]}\n\n",
            "data: [DONE]\n\n"
        );
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    #[test]
    fn urls_are_joined_without_double_slash() {
        let provider = OpenAiCompatProvider::new(
            config_with("https://api.example.com/v1/".into()),
            Some("k".into()),
        );
        assert_eq!(provider.models_url(), "https://api.example.com/v1/models");
        assert_eq!(
            provider.chat_url(),
            "https://api.example.com/v1/chat/completions"
        );
    }

    #[test]
    fn build_chat_body_matches_openai_schema() {
        let body = build_chat_body(&config_with("https://x".into()), &request());
        assert_eq!(body["model"], "test-model");
        assert_eq!(body["stream"], true);
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"], "你好");
    }

    #[test]
    fn build_chat_body_falls_back_to_configured_model() {
        let mut empty_model = request();
        empty_model.model = "  ".into();

        let body = build_chat_body(&config_with("https://x".into()), &empty_model);
        assert_eq!(body["model"], "test-model", "空模型名应回退到配置模型");
    }

    #[test]
    fn parse_sse_line_handles_tokens_done_and_noise() {
        assert_eq!(
            parse_sse_line(r#"data: {"choices":[{"delta":{"content":"abc"}}]}"#),
            Some(ChatDelta::Token { text: "abc".into() })
        );
        assert_eq!(
            parse_sse_line("data: [DONE]"),
            Some(ChatDelta::Done { finish_reason: "stop".into() })
        );
        assert_eq!(parse_sse_line("event: ping"), None);
        assert_eq!(parse_sse_line("data:"), None);
        assert_eq!(parse_sse_line(r#"data: {"choices":[{"delta":{}}]}"#), None);
    }

    #[test]
    fn parse_sse_line_surfaces_api_errors() {
        let delta = parse_sse_line(r#"data: {"error":{"message":"invalid key"}}"#);
        assert_eq!(
            delta,
            Some(ChatDelta::Error { message: "invalid key".into() })
        );
    }

    #[test]
    fn parse_sse_line_reports_finish_reason() {
        assert_eq!(
            parse_sse_line(r#"data: {"choices":[{"delta":{},"finish_reason":"length"}]}"#),
            Some(ChatDelta::Done { finish_reason: "length".into() })
        );
    }

    #[test]
    fn test_connection_succeeds_against_mock_server() {
        let (base, handle) = spawn_server(models_response);
        let provider = OpenAiCompatProvider::new(config_with(base), Some("k".into()));

        let result = provider.test_connection().expect("test");
        assert!(result.ok, "mock 服务应返回成功：{result:?}");
        assert!(result.message.contains("200"));
        handle.join().ok();
    }

    #[test]
    fn stream_chat_parses_sse_stream() {
        let (base, handle) = spawn_server(sse_response);
        let provider = OpenAiCompatProvider::new(config_with(base), Some("k".into()));

        let stream = provider.stream_chat(request()).expect("stream");
        let deltas: Vec<ChatDelta> = stream.iter().collect();

        assert_eq!(deltas.first(), Some(&ChatDelta::Start));
        assert!(deltas
            .iter()
            .any(|d| *d == ChatDelta::Token { text: "你好".into() }));
        assert!(deltas
            .iter()
            .any(|d| *d == ChatDelta::Token { text: "，世界".into() }));
        assert_eq!(
            deltas.last(),
            Some(&ChatDelta::Done { finish_reason: "stop".into() })
        );
        handle.join().ok();
    }

    #[test]
    fn test_connection_reports_server_error() {
        let (base, handle) = spawn_server(|_| {
            let body = r#"{"error":{"message":"invalid api key"}}"#;
            format!(
                "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
        });
        let provider = OpenAiCompatProvider::new(config_with(base), Some("bad".into()));

        let result = provider.test_connection().expect("test");
        assert!(!result.ok);
        assert!(result.message.contains("invalid api key"));
        handle.join().ok();
    }

    // ==================== ST-12 思考 / 非思考模式 ====================

    fn collect(provider: &OpenAiCompatProvider) -> Vec<ChatDelta> {
        provider
            .stream_chat(request())
            .expect("stream")
            .iter()
            .collect()
    }

    #[test]
    fn qwen_non_thinking_sends_disable_flag() {
        let (base, handle) = spawn_sequence_server(vec![sse_response("")]);
        let provider = OpenAiCompatProvider::new(
            config_for(AiPlatform::Qwen, ThinkingMode::NonThinking, base),
            Some("k".into()),
        );

        collect(&provider);

        let requests = handle.join().expect("server");
        assert_eq!(requests.len(), 1);
        assert!(
            requests[0].contains(r#""enable_thinking":false"#),
            "非思考模式应下发 enable_thinking=false：{}",
            requests[0]
        );
    }

    #[test]
    fn qwen_thinking_sends_enable_flag() {
        let (base, handle) = spawn_sequence_server(vec![sse_response("")]);
        let provider = OpenAiCompatProvider::new(
            config_for(AiPlatform::Qwen, ThinkingMode::Thinking, base),
            Some("k".into()),
        );

        collect(&provider);

        let requests = handle.join().expect("server");
        assert!(
            requests[0].contains(r#""enable_thinking":true"#),
            "思考模式应下发 enable_thinking=true：{}",
            requests[0]
        );
    }

    #[test]
    fn glm_thinking_uses_nested_type_flag() {
        let (base, handle) = spawn_sequence_server(vec![sse_response("")]);
        let provider = OpenAiCompatProvider::new(
            config_for(AiPlatform::Glm, ThinkingMode::Thinking, base),
            Some("k".into()),
        );

        collect(&provider);

        let requests = handle.join().expect("server");
        assert!(
            requests[0].contains(r#""thinking":{"type":"enabled"}"#),
            "GLM 应下发 thinking.type=enabled：{}",
            requests[0]
        );
    }

    #[test]
    fn platforms_without_switch_keep_body_unchanged() {
        let (base, handle) = spawn_sequence_server(vec![sse_response("")]);
        let provider = OpenAiCompatProvider::new(
            config_for(AiPlatform::Deepseek, ThinkingMode::Thinking, base),
            Some("k".into()),
        );

        collect(&provider);

        let requests = handle.join().expect("server");
        assert!(
            !requests[0].contains("thinking"),
            "DeepSeek 无思考开关参数，请求体不应出现 thinking：{}",
            requests[0]
        );
    }

    #[test]
    fn rejected_thinking_param_is_retried_without_it() {
        let (base, handle) = spawn_sequence_server(vec![
            error_response(400, "Unsupported parameter: enable_thinking"),
            sse_response(""),
        ]);
        let provider = OpenAiCompatProvider::new(
            config_for(AiPlatform::Qwen, ThinkingMode::NonThinking, base),
            Some("k".into()),
        );

        let deltas = collect(&provider);
        assert!(
            deltas
                .iter()
                .any(|delta| *delta == ChatDelta::Token { text: "你好".into() }),
            "去参重试后应正常收到回答：{deltas:?}"
        );

        let requests = handle.join().expect("server");
        assert_eq!(requests.len(), 2, "应重试一次");
        assert!(requests[0].contains("enable_thinking"));
        assert!(
            !requests[1].contains("enable_thinking"),
            "重试请求不应再带思考参数：{}",
            requests[1]
        );
    }

    #[test]
    fn unrelated_error_is_not_retried() {
        let (base, handle) = spawn_sequence_server(vec![error_response(400, "Insufficient balance")]);
        let provider = OpenAiCompatProvider::new(
            config_for(AiPlatform::Qwen, ThinkingMode::NonThinking, base),
            Some("k".into()),
        );

        let err = provider.stream_chat(request()).expect_err("应直接报错");
        assert!(err.to_string().contains("Insufficient balance"), "{err}");

        let requests = handle.join().expect("server");
        assert_eq!(requests.len(), 1, "与思考参数无关的错误不应重试");
    }
}
