//! 阿里云百炼（DashScope）实时语音识别 WebSocket 客户端（IA-16）。
//!
//! 协议要点（对照官方「音频转写交互协议(WebSocket)」）：
//! * 地址：默认 `wss://dashscope.aliyuncs.com/api-ws/v1/inference`，可由设置页 Base URL 覆盖；
//! * 鉴权：建连请求头 `Authorization: Bearer <api_key>`；
//! * 指令：JSON 文本帧 `run-task` → 服务端 `task-started` → 连续二进制帧送音频
//!   → `finish-task` → 服务端 `task-finished`；
//! * 结果：`result-generated` 的 `payload.output.sentence`（`text` + `sentence_end`）；
//! * 音频：单声道 16bit PCM、16kHz（本模块负责 f32 → PCM16 LE 转换）。
//!
//! 线程模型：引擎独占一个**专职线程 + 单线程 tokio runtime**，`tokio::select!` 在
//! 「待发送音频」与「WS 读流」之间公平调度；上层线程只做非阻塞投递与拉取。

use super::{CloudAsrEngine, CloudTranscript, EngineSpec};
use crate::providers::types::{ProviderError, ProviderResult};
use futures_util::{SinkExt, StreamExt};
use std::sync::mpsc as std_mpsc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc as tokio_mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::{HeaderName, HeaderValue};
use tokio_tungstenite::tungstenite::Message;

/// 引擎标识（`asr_status.engine` 展示）
pub const ENGINE_NAME: &str = "aliyun-realtime";

/// 连接 + 握手超时（工作线程内部）
const CONNECT_TIMEOUT_MS: u64 = 8_000;
/// 主线程等待握手结果的超时（略大于内部超时）
const HANDSHAKE_WAIT_MS: u64 = 10_000;
/// `finish()` 等待服务端 `task-finished` 的超时
const FINISH_TIMEOUT_MS: u64 = 3_000;
/// 音频聚合目标：1600 样本 = 100ms @16kHz
const SEND_BATCH_SAMPLES: usize = 1_600;
/// 音频投递队列容量（满则丢帧而不是阻塞采集线程）
const AUDIO_QUEUE: usize = 128;

type WsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// 工作线程指令
enum WorkerMsg {
    Audio(Vec<f32>),
    Finish,
}

/// 工作线程回传事件
enum WorkerEvent {
    Transcript(CloudTranscript),
    Error(String),
}

/// 服务端事件（完整协议以官方文档为准，解析对未知字段保持宽容）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerEvent {
    /// `task-started`
    Started,
    /// `result-generated`：句级识别结果；`is_final` 对应 `sentence_end`
    Result { text: String, is_final: bool },
    /// `task-finished`
    Finished,
    /// `task-failed`
    Failed { code: String, message: String },
    /// 其他/无法解析
    Other,
}

/// 生成合法的 `task_id`（字母数字下划线，长度 ≤ 32）
pub fn new_task_id() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("ei_{now:x}")
}

/// `run-task` 指令（JSON 文本帧）
pub fn build_run_task(model: &str, task_id: &str) -> String {
    serde_json::json!({
        "header": { "action": "run-task", "task_id": task_id, "streaming": "duplex" },
        "payload": {
            "task_group": "audio",
            "task": "asr",
            "function": "recognition",
            "model": model,
            "parameters": { "format": "pcm", "sample_rate": 16_000 },
            "input": {}
        }
    })
    .to_string()
}

/// `finish-task` 指令（JSON 文本帧）
pub fn build_finish_task(task_id: &str) -> String {
    serde_json::json!({
        "header": { "action": "finish-task", "task_id": task_id, "streaming": "duplex" },
        "payload": { "input": {} }
    })
    .to_string()
}

/// f32（-1.0 ~ 1.0）→ 16bit 小端 PCM 字节流
pub fn pcm16_le(samples: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() * 2);
    for &sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

/// 解析服务端文本帧
pub fn parse_server_event(raw: &str) -> ServerEvent {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return ServerEvent::Other;
    };
    let event = value
        .pointer("/header/event")
        .and_then(|value| value.as_str())
        .unwrap_or_default();

    match event {
        "task-started" => ServerEvent::Started,
        "task-finished" => ServerEvent::Finished,
        "task-failed" => ServerEvent::Failed {
            code: value
                .pointer("/header/error_code")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown")
                .to_string(),
            message: value
                .pointer("/header/error_message")
                .and_then(|value| value.as_str())
                .unwrap_or("服务端未提供错误详情")
                .to_string(),
        },
        "result-generated" => {
            let text = value
                .pointer("/payload/output/sentence/text")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string();
            let is_final = value
                .pointer("/payload/output/sentence/sentence_end")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            if text.trim().is_empty() {
                ServerEvent::Other
            } else {
                ServerEvent::Result { text, is_final }
            }
        }
        _ => ServerEvent::Other,
    }
}

/// 去掉 URL 中的查询串，避免把签名类参数写进日志
fn redact_url(url: &str) -> String {
    match url.split_once('?') {
        Some((head, _)) => format!("{head}?..."),
        None => url.to_string(),
    }
}

/// 阿里云百炼实时识别引擎
pub struct AliyunRealtimeEngine {
    engine_name: String,
    task_id: String,
    tx: Option<tokio_mpsc::Sender<WorkerMsg>>,
    results: std_mpsc::Receiver<WorkerEvent>,
    pending: Vec<CloudTranscript>,
    /// 已收到的致命错误（下次 `drain` 上报一次）
    failure: Option<String>,
    done_rx: Option<std_mpsc::Receiver<()>>,
    handle: Option<JoinHandle<()>>,
    finished: bool,
}

impl AliyunRealtimeEngine {
    /// 建立连接并完成 `task-started` 握手（阻塞；失败返回可操作错误）
    pub fn connect(spec: EngineSpec) -> ProviderResult<Self> {
        let api_key = spec.api_key.trim().to_string();
        if api_key.is_empty() {
            return Err(ProviderError::Other(format!(
                "{}：尚未配置 API Key，请到「设置 → 语音识别」填写服务商凭证后再开始转写。",
                spec.provider.label()
            )));
        }

        let url = spec.websocket_url();
        if !url.starts_with("ws://") && !url.starts_with("wss://") {
            return Err(ProviderError::Other(format!(
                "服务端地址不是合法的 WebSocket 地址：{url}（应以 ws:// 或 wss:// 开头）"
            )));
        }

        let task_id = new_task_id();
        let run_task = build_run_task(&spec.model, &task_id);
        let channel = spec.channel.clone();

        let (msg_tx, msg_rx) = tokio_mpsc::channel::<WorkerMsg>(AUDIO_QUEUE);
        let (event_tx, event_rx) = std_mpsc::channel::<WorkerEvent>();
        let (ready_tx, ready_rx) = std_mpsc::channel::<Result<(), String>>();
        let (done_tx, done_rx) = std_mpsc::channel::<()>();

        let thread_url = url.clone();
        let handle = thread::Builder::new()
            .name(format!("asr-cloud-{channel}"))
            .spawn(move || {
                worker_main(
                    thread_url, api_key, run_task, msg_rx, event_tx, ready_tx, done_tx,
                )
            })
            .map_err(|err| ProviderError::Other(format!("启动云端 ASR 线程失败：{err}")))?;

        match ready_rx.recv_timeout(Duration::from_millis(HANDSHAKE_WAIT_MS)) {
            Ok(Ok(())) => Ok(Self {
                engine_name: ENGINE_NAME.to_string(),
                task_id,
                tx: Some(msg_tx),
                results: event_rx,
                pending: Vec::new(),
                failure: None,
                done_rx: Some(done_rx),
                handle: Some(handle),
                finished: false,
            }),
            Ok(Err(message)) => {
                drop(msg_tx);
                let _ = handle.join();
                Err(ProviderError::Network(message))
            }
            Err(_) => {
                // 握手超时：关闭投递通道促使工作线程退出（内部亦有超时，绝不悬挂）
                drop(msg_tx);
                Err(ProviderError::Network(format!(
                    "连接云端 ASR 超时（{}ms），请检查网络与服务端地址：{}",
                    HANDSHAKE_WAIT_MS,
                    redact_url(&url)
                )))
            }
        }
    }

    /// 非阻塞拉取工作线程事件
    fn poll(&mut self) -> ProviderResult<Vec<CloudTranscript>> {
        let mut transcripts = std::mem::take(&mut self.pending);
        while let Ok(event) = self.results.try_recv() {
            match event {
                WorkerEvent::Transcript(transcript) => transcripts.push(transcript),
                WorkerEvent::Error(message) => {
                    if self.failure.is_none() {
                        self.failure = Some(message);
                    }
                }
            }
        }

        if transcripts.is_empty() {
            if let Some(message) = self.failure.take() {
                return Err(ProviderError::Network(message));
            }
        }
        Ok(transcripts)
    }

    /// 关闭投递通道并等待工作线程退出
    fn shutdown(&mut self) {
        self.tx.take();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl CloudAsrEngine for AliyunRealtimeEngine {
    fn name(&self) -> &str {
        &self.engine_name
    }

    fn feed(&mut self, frame: &[f32]) -> ProviderResult<()> {
        if frame.is_empty() || self.finished || self.tx.is_none() {
            return Ok(());
        }
        let Some(tx) = self.tx.as_ref() else {
            return Ok(());
        };
        match tx.try_send(WorkerMsg::Audio(frame.to_vec())) {
            Ok(()) => Ok(()),
            // 队列已满：丢帧而非阻塞采集线程（音频流可容忍偶发丢帧）
            Err(tokio_mpsc::error::TrySendError::Full(_)) => {
                tracing::debug!(task_id = %self.task_id, "云端 ASR 音频队列已满，丢弃一帧");
                Ok(())
            }
            Err(tokio_mpsc::error::TrySendError::Closed(_)) => {
                Err(ProviderError::Network("云端 ASR 连接已关闭".into()))
            }
        }
    }

    fn drain(&mut self) -> ProviderResult<Vec<CloudTranscript>> {
        self.poll()
    }

    fn finish(&mut self) -> ProviderResult<Vec<CloudTranscript>> {
        if self.finished {
            return self.poll();
        }
        self.finished = true;

        // 1) 请求结束任务（队列满时退化为阻塞投递；通道已关闭则忽略）
        if let Some(tx) = self.tx.as_ref() {
            match tx.try_send(WorkerMsg::Finish) {
                Ok(()) => {}
                Err(tokio_mpsc::error::TrySendError::Full(message)) => {
                    let _ = tx.blocking_send(message);
                }
                Err(tokio_mpsc::error::TrySendError::Closed(_)) => {}
            }
        }

        // 2) 等待服务端 task-finished（超时后强制收尾）
        if let Some(done_rx) = self.done_rx.take() {
            let _ = done_rx.recv_timeout(Duration::from_millis(FINISH_TIMEOUT_MS));
        }

        // 3) 关闭通道并 join 工作线程（必定回收线程与连接）
        self.shutdown();

        let mut transcripts = self.poll().unwrap_or_default();
        transcripts.extend(self.pending.drain(..));
        Ok(transcripts)
    }
}

impl Drop for AliyunRealtimeEngine {
    fn drop(&mut self) {
        // 关闭投递通道 → 工作线程的 select 立即返回 None 并退出
        self.shutdown();
    }
}

/// 工作线程入口：建立单线程 runtime 并执行会话
fn worker_main(
    url: String,
    api_key: String,
    run_task: String,
    msg_rx: tokio_mpsc::Receiver<WorkerMsg>,
    events: std_mpsc::Sender<WorkerEvent>,
    ready_tx: std_mpsc::Sender<Result<(), String>>,
    done_tx: std_mpsc::Sender<()>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            let _ = ready_tx.send(Err(format!("初始化云端 ASR 运行时失败：{err}")));
            let _ = done_tx.send(());
            return;
        }
    };

    runtime.block_on(session(url, api_key, run_task, msg_rx, events, ready_tx));
    let _ = done_tx.send(());
}

/// 一次完整会话：连接握手 → 双向循环（送音频 / 收结果）→ 结束
async fn session(
    url: String,
    api_key: String,
    run_task: String,
    mut msg_rx: tokio_mpsc::Receiver<WorkerMsg>,
    events: std_mpsc::Sender<WorkerEvent>,
    ready_tx: std_mpsc::Sender<Result<(), String>>,
) {
    // 1) 连接 + 握手（带超时；失败通过 ready 通道回传给上层）
    let mut ws = match tokio::time::timeout(
        Duration::from_millis(CONNECT_TIMEOUT_MS),
        connect_and_start(&url, &api_key, &run_task),
    )
    .await
    {
        Ok(Ok(ws)) => {
            let _ = ready_tx.send(Ok(()));
            ws
        }
        Ok(Err(message)) => {
            let _ = ready_tx.send(Err(message));
            return;
        }
        Err(_) => {
            let _ = ready_tx.send(Err(format!(
                "连接云端 ASR 超时（{}ms）：{}",
                CONNECT_TIMEOUT_MS,
                redact_url(&url)
            )));
            return;
        }
    };

    // 2) 双向循环
    let mut buffer: Vec<f32> = Vec::new();
    let mut finish_requested = false;
    let task_id = new_task_id_of(&run_task);

    loop {
        tokio::select! {
            message = msg_rx.recv() => match message {
                None => break,
                Some(WorkerMsg::Audio(samples)) => {
                    if finish_requested {
                        continue;
                    }
                    buffer.extend_from_slice(&samples);
                    while buffer.len() >= SEND_BATCH_SAMPLES {
                        let batch: Vec<f32> = buffer.drain(..SEND_BATCH_SAMPLES).collect();
                        if let Err(err) = ws.send(Message::binary(pcm16_le(&batch))).await {
                            emit_error(&events, format!("发送音频失败：{err}"));
                            return;
                        }
                    }
                }
                Some(WorkerMsg::Finish) => {
                    if finish_requested {
                        continue;
                    }
                    finish_requested = true;
                    if !buffer.is_empty() {
                        let batch = std::mem::take(&mut buffer);
                        let _ = ws.send(Message::binary(pcm16_le(&batch))).await;
                    }
                    if let Err(err) = ws.send(Message::text(build_finish_task(&task_id))).await {
                        emit_error(&events, format!("发送 finish-task 失败：{err}"));
                        return;
                    }
                }
            },
            incoming = ws.next() => match incoming {
                Some(Ok(Message::Text(text))) => match parse_server_event(text.as_str()) {
                    ServerEvent::Result { text, is_final } => {
                        let _ = events.send(WorkerEvent::Transcript(CloudTranscript {
                            text,
                            is_final,
                        }));
                    }
                    ServerEvent::Failed { code, message } => {
                        emit_error(&events, failed_message(&code, &message));
                        return;
                    }
                    ServerEvent::Finished => return,
                    _ => {}
                },
                Some(Ok(Message::Close(_))) | None => return,
                Some(Ok(_)) => {}
                Some(Err(err)) => {
                    emit_error(&events, format!("云端 ASR 连接错误：{err}"));
                    return;
                }
            },
        }
    }

    let _ = ws.close(None).await;
}

/// 从 run-task JSON 中取回 task_id（会话内部复用；解析失败时回退新 id）
fn new_task_id_of(run_task: &str) -> String {
    serde_json::from_str::<serde_json::Value>(run_task)
        .ok()
        .and_then(|value| {
            value
                .pointer("/header/task_id")
                .and_then(|value| value.as_str())
                .map(|value| value.to_string())
        })
        .unwrap_or_else(new_task_id)
}

fn emit_error(events: &std_mpsc::Sender<WorkerEvent>, message: String) {
    let _ = events.send(WorkerEvent::Error(message));
}

/// 握手失败的错误映射：带上**实际使用的地址**与可操作提示
fn map_handshake_error(url: &str, err: tokio_tungstenite::tungstenite::Error) -> String {
    use tokio_tungstenite::tungstenite::http::StatusCode;

    let address = redact_url(url);
    if let tokio_tungstenite::tungstenite::Error::Http(ref response) = err {
        let status = response.status();
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            return format!(
                "鉴权失败（HTTP {}，地址：{address}）：请检查「设置 → 语音识别」中的 API Key 是否正确、是否已过期",
                status.as_u16()
            );
        }
        if status == StatusCode::NOT_FOUND {
            return format!(
                "服务端返回 HTTP 404（地址：{address}）：该地址不是实时识别端点（实时转写走 WebSocket）。若填的是 OpenAI 兼容的 /compatible-mode/v1 等 HTTP 接口地址，请改为 {}，或点「恢复默认」后保存配置",
                crate::config::asr::CloudAsrProvider::Aliyun.default_base_url()
            );
        }
        return format!("连接云端 ASR 失败：HTTP {}（地址：{address}）", status.as_u16());
    }
    format!("连接云端 ASR 失败：{err}（地址：{address}）")
}

/// 服务端 `task-failed` 的可操作文案（模型名不适用于实时端点是最常见的自伤）
fn failed_message(code: &str, message: &str) -> String {
    let mut text = format!("服务端拒绝任务（{code}）：{message}");
    if code.eq_ignore_ascii_case("ModelNotFound") || message.contains("Model not found") {
        text.push_str(
            "。请检查云端模型名：① 非实时（HTTP 文件转写）模型不能用于实时转写，例如 `qwen-audio-3.1-asr-flash` 的实时版是 `qwen-audio-3.1-asr-flash-streaming`；② 确认模型名拼写正确且在当前地域/账号可用。实测可用的实时模型：qwen-audio-3.1-asr-flash-streaming、fun-asr-realtime、paraformer-realtime-v2",
        );
    }
    text
}

/// 建立连接并等待 `task-started`
async fn connect_and_start(url: &str, api_key: &str, run_task: &str) -> Result<WsStream, String> {
    // 注意：必须用 `into_client_request()` 生成请求，它负责补齐 WebSocket 握手头
    // （`Sec-WebSocket-Key` 等）；手工 `Request::builder()` 构造会因缺少握手头而被拒。
    let mut request = url
        .into_client_request()
        .map_err(|err| format!("构造 WebSocket 请求失败：{err}"))?;
    let credential = HeaderValue::from_str(&format!("Bearer {api_key}"))
        .map_err(|_| "API Key 含非法字符，无法用于鉴权请求头".to_string())?;
    request
        .headers_mut()
        .insert(HeaderName::from_static("authorization"), credential);

    let (mut ws, _response) = tokio_tungstenite::connect_async(request)
        .await
        .map_err(|err| map_handshake_error(url, err))?;

    ws.send(Message::text(run_task))
        .await
        .map_err(|err| format!("发送 run-task 指令失败：{err}"))?;

    loop {
        match ws.next().await {
            Some(Ok(Message::Text(text))) => match parse_server_event(text.as_str()) {
                ServerEvent::Started => return Ok(ws),
                ServerEvent::Failed { code, message } => {
                    return Err(failed_message(&code, &message))
                }
                _ => continue,
            },
            Some(Ok(Message::Close(_))) | None => {
                return Err("服务端在握手阶段关闭了连接（请检查 API Key 与服务端地址）".into())
            }
            Some(Ok(_)) => continue,
            Some(Err(err)) => return Err(format!("握手阶段连接错误：{err}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::asr::CloudAsrProvider;

    fn spec(base_url: &str, api_key: &str) -> EngineSpec {
        EngineSpec {
            provider: CloudAsrProvider::Aliyun,
            base_url: base_url.to_string(),
            // 用「出厂默认模型」，确保联调用例验证的就是用户默认拿到的模型
            model: CloudAsrProvider::Aliyun.default_model().to_string(),
            language: "zh-CN".to_string(),
            api_key: api_key.to_string(),
            channel: "mic".to_string(),
        }
    }

    /// 联调用规格：模型可用 `ASR_MODEL` 覆盖（默认取出厂默认的实时模型）
    fn real_spec(base_url: &str, api_key: &str) -> EngineSpec {
        let mut spec = spec(base_url, api_key);
        if let Ok(model) = std::env::var("ASR_MODEL") {
            if !model.trim().is_empty() {
                spec.model = model;
            }
        }
        spec
    }

    /// 连接成功（引擎类型不含 Debug，故不用 `expect`）
    fn connect_ok(spec: EngineSpec) -> AliyunRealtimeEngine {
        match AliyunRealtimeEngine::connect(spec) {
            Ok(engine) => engine,
            Err(err) => panic!("应连接成功：{err}"),
        }
    }

    /// 连接失败的错误文案
    fn connect_error(spec: EngineSpec) -> String {
        match AliyunRealtimeEngine::connect(spec) {
            Ok(_) => panic!("预期连接失败"),
            Err(err) => err.to_string(),
        }
    }

    #[test]
    fn run_task_carries_model_and_pcm_parameters() {
        let json: serde_json::Value =
            serde_json::from_str(&build_run_task("gummy-realtime-v1", "ei_1")).expect("json");
        assert_eq!(json["header"]["action"], "run-task");
        assert_eq!(json["header"]["task_id"], "ei_1");
        assert_eq!(json["header"]["streaming"], "duplex");
        assert_eq!(json["payload"]["task"], "asr");
        assert_eq!(json["payload"]["function"], "recognition");
        assert_eq!(json["payload"]["model"], "gummy-realtime-v1");
        assert_eq!(json["payload"]["parameters"]["format"], "pcm");
        assert_eq!(json["payload"]["parameters"]["sample_rate"], 16_000);
    }

    #[test]
    fn finish_task_uses_same_task_id() {
        let json: serde_json::Value =
            serde_json::from_str(&build_finish_task("ei_2")).expect("json");
        assert_eq!(json["header"]["action"], "finish-task");
        assert_eq!(json["header"]["task_id"], "ei_2");
    }

    #[test]
    fn task_id_is_url_safe_and_short() {
        let task_id = new_task_id();
        assert!(task_id.len() <= 32, "task_id 过长：{task_id}");
        assert!(task_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_'));
    }

    #[test]
    fn pcm16_le_clamps_and_encodes_little_endian() {
        let bytes = pcm16_le(&[0.0, 1.0, -1.0, 2.0]);
        assert_eq!(bytes.len(), 8);
        assert_eq!(&bytes[0..2], &0i16.to_le_bytes());
        assert_eq!(&bytes[2..4], &i16::MAX.to_le_bytes());
        assert_eq!(&bytes[4..6], &(-i16::MAX).to_le_bytes());
        // 超出范围应被钳制而不是溢出
        assert_eq!(&bytes[6..8], &i16::MAX.to_le_bytes());
    }

    #[test]
    fn parse_started_finished_and_failed() {
        assert_eq!(
            parse_server_event(r#"{"header":{"event":"task-started"}}"#),
            ServerEvent::Started
        );
        assert_eq!(
            parse_server_event(r#"{"header":{"event":"task-finished"}}"#),
            ServerEvent::Finished
        );
        assert_eq!(
            parse_server_event(
                r#"{"header":{"event":"task-failed","error_code":"InvalidParameter","error_message":"model not found"}}"#
            ),
            ServerEvent::Failed {
                code: "InvalidParameter".into(),
                message: "model not found".into()
            }
        );
        assert_eq!(parse_server_event("not json"), ServerEvent::Other);
        assert_eq!(
            parse_server_event(r#"{"header":{"event":"unknown-event"}}"#),
            ServerEvent::Other
        );
    }

    #[test]
    fn parse_result_sentence_maps_partial_and_final() {
        assert_eq!(
            parse_server_event(
                r#"{"header":{"event":"result-generated"},"payload":{"output":{"sentence":{"text":"面试官在问","sentence_end":false}}}}"#
            ),
            ServerEvent::Result {
                text: "面试官在问".into(),
                is_final: false
            }
        );
        assert_eq!(
            parse_server_event(
                r#"{"header":{"event":"result-generated"},"payload":{"output":{"sentence":{"text":"面试官在问问题","sentence_end":true}}}}"#
            ),
            ServerEvent::Result {
                text: "面试官在问问题".into(),
                is_final: true
            }
        );
        // 空文本不产出事件
        assert_eq!(
            parse_server_event(
                r#"{"header":{"event":"result-generated"},"payload":{"output":{"sentence":{"text":"","sentence_end":false}}}}"#
            ),
            ServerEvent::Other
        );
    }

    #[test]
    fn connect_requires_api_key() {
        let message = connect_error(spec("wss://example.com/ws", "  "));
        assert!(message.contains("API Key"), "{message}");
        assert!(message.contains("设置"), "应给出可操作引导：{message}");
    }

    #[test]
    fn connect_rejects_non_websocket_url() {
        let message = connect_error(spec("ftp://example.com/ws", "key"));
        assert!(message.contains("WebSocket"));
    }

    /// 端到端：mock 服务端驱动完整协议时序，partial → final 且 final 落为句末结果
    #[test]
    fn end_to_end_with_mock_server_emits_partial_then_final() {
        let server = super::super::mock::MockServer::start(super::super::mock::MockConfig::normal());
        let mut engine = connect_ok(spec(&server.websocket_url(), "test-key"));

        let mut transcripts = Vec::new();
        // 12 帧 × 20ms = 240ms 语音 → 触发 mock 的中间结果
        for _ in 0..12 {
            engine
                .feed(&vec![0.2f32; crate::audio::FRAME_SAMPLES])
                .expect("feed");
            transcripts.extend(engine.drain().expect("drain"));
        }
        transcripts.extend(engine.finish().expect("finish"));

        let partials: Vec<&CloudTranscript> =
            transcripts.iter().filter(|t| !t.is_final).collect();
        let finals: Vec<&CloudTranscript> =
            transcripts.iter().filter(|t| t.is_final).collect();
        assert!(!partials.is_empty(), "应产出 partial：{transcripts:?}");
        assert_eq!(finals.len(), 1, "应恰好产出一个 final：{transcripts:?}");
        assert_eq!(finals[0].text, "面试官在问问题");
    }

    /// 鉴权失败：握手阶段以可操作错误返回（不产生任何转写）
    #[test]
    fn auth_failure_is_surfaced_during_handshake() {
        let server = super::super::mock::MockServer::start(
            super::super::mock::MockConfig::auth_failed(),
        );
        let message = connect_error(spec(&server.websocket_url(), "bad-key"));
        assert!(message.contains("服务端拒绝任务"), "{message}");
        assert!(message.contains("InvalidApiKey"), "{message}");
    }

    /// 真实凭证联调（默认 `#[ignore]`）：验证「建连 + 鉴权 + task-started 握手」。
    ///
    /// 运行：`$env:DASHSCOPE_API_KEY="sk-xxx"; cargo test --lib -- --ignored real_aliyun`
    /// 说明：不发音频、不落库；仅确认地址与凭证可用（计费以服务商口径为准）。
    #[test]
    #[ignore = "需要真实 DASHSCOPE_API_KEY（cargo test -- --ignored real_aliyun）"]
    fn real_aliyun_handshake_succeeds_with_env_api_key() {
        let api_key = std::env::var("DASHSCOPE_API_KEY")
            .expect("请先设置环境变量 DASHSCOPE_API_KEY 再运行该联调用例");
        let mut engine = connect_ok(real_spec(
            CloudAsrProvider::Aliyun.default_base_url(),
            &api_key,
        ));
        assert_eq!(engine.name(), ENGINE_NAME);
        // 立即结束任务，回收连接
        let _ = engine.finish().expect("结束任务");
    }

    /// 真实凭证 + 真实音频链路联调（默认 `#[ignore]`）。
    ///
    /// 运行：
    /// ```powershell
    /// $env:DASHSCOPE_API_KEY="sk-xxx"
    /// # 可选：给一段 16k 单声道 WAV 做真实语音识别
    /// $env:ASR_SAMPLE_WAV="D:\sample-16k.wav"
    /// cargo test --lib -- --ignored real_aliyun --nocapture
    /// ```
    /// 断言：音频帧投递、结果拉取与 `finish-task` 收尾**均无错误**；
    /// 提供 `ASR_SAMPLE_WAV` 时进一步断言能识别出非空文本（否则只验证协议通路，
    /// 因合成音频不保证被识别为有效语音）。
    #[test]
    #[ignore = "需要真实 DASHSCOPE_API_KEY（cargo test -- --ignored real_aliyun）"]
    fn real_aliyun_audio_round_trip_sends_audio_and_finishes_cleanly() {
        let api_key = std::env::var("DASHSCOPE_API_KEY")
            .expect("请先设置环境变量 DASHSCOPE_API_KEY 再运行该联调用例");
        let mut engine = connect_ok(real_spec(
            CloudAsrProvider::Aliyun.default_base_url(),
            &api_key,
        ));

        let sample_path = std::env::var("ASR_SAMPLE_WAV").ok();
        let samples: Vec<f32> = match sample_path.as_deref() {
            Some(path) => {
                crate::audio::wav::read_wav_as_16k_mono(std::path::Path::new(path))
                    .expect("读取样例 WAV 失败")
            }
            // 合成 1.2s 正弦（仅用于打通音频帧 → 服务端的协议通路）
            None => (0..crate::audio::TARGET_SAMPLE_RATE as usize * 12 / 10)
                .map(|index| 0.2 * ((index as f32) * 0.08).sin())
                .collect(),
        };

        let mut transcripts = Vec::new();
        for frame in samples.chunks(crate::audio::FRAME_SAMPLES) {
            engine.feed(frame).expect("投递音频失败");
            transcripts.extend(engine.drain().expect("拉取识别结果失败"));
            // 按实时率节流：服务端实时识别不适合过快灌入
            std::thread::sleep(Duration::from_millis(20));
        }
        transcripts.extend(engine.finish().expect("结束任务失败"));

        let finals: Vec<&str> = transcripts
            .iter()
            .filter(|transcript| transcript.is_final)
            .map(|transcript| transcript.text.as_str())
            .collect();
        eprintln!(
            "[联调] 结果 {} 条（final {} 条）：{:?}",
            transcripts.len(),
            finals.len(),
            finals
        );

        if sample_path.is_some() {
            assert!(
                !finals.is_empty() && finals.iter().any(|text| !text.trim().is_empty()),
                "提供真实语音样例时应识别出非空文本"
            );
        }
    }

    /// 错误凭证 / 无效地址必须得到**可操作错误**，而不是静默成功（默认 `#[ignore]`）。
    #[test]
    #[ignore = "需要外网（cargo test -- --ignored real_aliyun）"]
    fn real_aliyun_invalid_key_is_rejected() {
        let message = connect_error(spec(
            CloudAsrProvider::Aliyun.default_base_url(),
            "sk-invalid-key-for-error-path-test",
        ));
        eprintln!("[联调] 无效凭证的错误文案：{message}");
        assert!(
            message.contains("鉴权失败") || message.contains("服务端拒绝任务"),
            "应给出可操作的鉴权错误：{message}"
        );
    }

    #[test]
    fn handshake_error_maps_status_codes_with_used_address() {
        use tokio_tungstenite::tungstenite::http::Response;
        use tokio_tungstenite::tungstenite::Error;

        let http_error = |status: u16| {
            Error::Http(
                Response::builder()
                    .status(status)
                    .body(None::<Vec<u8>>)
                    .expect("response"),
            )
        };
        const URL: &str = "wss://example.com/api-ws/v1/inference";

        let message = map_handshake_error(URL, http_error(401));
        assert!(message.contains("鉴权失败"), "{message}");
        assert!(message.contains("401"), "{message}");
        assert!(message.contains("API Key"), "应给出可操作引导：{message}");
        assert!(message.contains("设置"), "{message}");
        assert!(message.contains(URL), "应回显实际使用的地址：{message}");

        assert!(map_handshake_error(URL, http_error(403)).contains("鉴权失败"));

        // 404：多为把 HTTP 接口地址（如 /compatible-mode/v1）当成实时识别端点
        let message = map_handshake_error(
            "wss://dashscope.aliyuncs.com/compatible-mode/v1/",
            http_error(404),
        );
        assert!(message.contains("404"), "{message}");
        assert!(message.contains("WebSocket"), "{message}");
        assert!(
            message.contains("compatible-mode"),
            "应回显实际使用的地址：{message}"
        );
        assert!(
            message.contains("api-ws/v1/inference"),
            "应给出正确的实时识别地址：{message}"
        );
        assert!(message.contains("恢复默认"), "{message}");

        let message = map_handshake_error(URL, http_error(503));
        assert!(message.contains("503"), "{message}");
        assert!(!message.contains("鉴权失败"), "{message}");
    }

    #[test]
    fn model_not_found_failure_gives_actionable_hint() {
        // 真实踩坑：非实时（HTTP）模型名不能用于实时端点
        let message = failed_message(
            "ModelNotFound",
            "Model not found (qwen-audio-3.1-asr-flash)!",
        );
        assert!(message.contains("服务端拒绝任务"), "{message}");
        assert!(message.contains("qwen-audio-3.1-asr-flash"), "{message}");
        assert!(
            message.contains("qwen-audio-3.1-asr-flash-streaming"),
            "应给出可用的实时模型：{message}"
        );
        assert!(message.contains("fun-asr-realtime"), "{message}");

        // 其他错误码不附加模型提示，避免噪音
        let other = failed_message("InvalidParameter", "bad parameter");
        assert!(!other.contains("qwen-audio-3.1-asr-flash-streaming"), "{other}");
    }

    /// 中途断连：以错误事件上报，而不是静默停止
    #[test]
    fn mid_session_drop_is_surfaced_as_error() {
        let server = super::super::mock::MockServer::start(
            super::super::mock::MockConfig::drop_after_audio(),
        );
        let mut engine = connect_ok(spec(&server.websocket_url(), "test-key"));

        let mut error: Option<String> = None;
        for _ in 0..40 {
            let _ = engine.feed(&vec![0.2f32; crate::audio::FRAME_SAMPLES]);
            match engine.drain() {
                Ok(_) => {}
                Err(err) => {
                    error = Some(err.to_string());
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(error.is_some(), "断连应上报错误");
    }
}
