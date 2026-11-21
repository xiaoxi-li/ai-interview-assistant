//! 腾讯云实时语音识别（asr/v2）WebSocket 客户端（IA-16.9 / TODO-10）。
//!
//! 协议要点（对照官方《实时语音识别（WebSocket）接口》文档）：
//! * 地址：默认 `wss://asr.cloud.tencent.com/asr/v2`，实际请求在其后拼接 `/{appid}`，
//!   可由设置页 Base URL 覆盖；
//! * 鉴权：HMAC-SHA1 签名拼入 URL 查询串（凭证 `app_id` + `secret_id` + `secret_key`）；
//! * 指令：**建连即识别**——直接连续发送二进制 PCM16 帧（40ms 一帧）；
//!   结束时发送一个 **0 字节空二进制帧**；
//! * 结果：JSON 文本帧 `result.voice_text_str` 为文本，`result.slice_type == 2` 表示句结束；
//! * 音频：单声道 16bit PCM、16kHz；
//! * 连接建好后进入双向循环，上层线程只做非阻塞投递与拉取（同阿里云线程模型）。

use super::{CloudAsrEngine, CloudTranscript, EngineSpec};
use crate::providers::types::{ProviderError, ProviderResult};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use hmac::{Hmac, Mac};
use sha1::Sha1;
use std::sync::mpsc as std_mpsc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc as tokio_mpsc;
use tokio_tungstenite::tungstenite::Message;

/// 引擎标识（`asr_status.engine` 展示）
pub const ENGINE_NAME: &str = "tencent-realtime";

/// 连接超时（工作线程内部）
const CONNECT_TIMEOUT_MS: u64 = 8_000;
/// 主线程等待握手结果的超时（略大于内部超时）
const HANDSHAKE_WAIT_MS: u64 = 10_000;
/// `finish()` 等待服务端收尾的超时
const FINISH_TIMEOUT_MS: u64 = 3_000;
/// 音频聚合目标：1280 样本 = 40ms @16kHz
const SEND_BATCH_SAMPLES: usize = 1_280;
/// 音频投递队列容量（满则丢帧而不是阻塞采集线程）
const AUDIO_QUEUE: usize = 128;
/// 签名有效期（秒）：当前时间起 1 小时
const SIGNATURE_EXPIRES_SECS: u64 = 3_600;

type HmacSha1 = Hmac<Sha1>;

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

/// 服务端事件
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerEvent {
    /// 识别结果：`slice_type == 2` 表示句结束（final）
    Result { text: String, is_final: bool },
    /// `code != 0`（签名 / 参数 / 配额等错误）
    Failed { code: i64, message: String },
    /// 其他 / 无法解析
    Other,
}

/// 仅 URL 查询串场景的百分号编码
fn url_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// 从 `ws(s)://host/path` 地址解析 `(scheme, host, path)`（结尾斜杠归一化）
fn split_ws_url(url: &str) -> Result<(String, String, String), String> {
    let (scheme, rest) = url
        .strip_prefix("wss://")
        .map(|rest| ("wss".to_string(), rest))
        .or_else(|| url.strip_prefix("ws://").map(|rest| ("ws".to_string(), rest)))
        .ok_or_else(|| {
            format!("服务端地址不是合法的 WebSocket 地址：{url}（应以 ws:// 或 wss:// 开头）")
        })?;
    let rest = rest.trim_end_matches('/');
    let (host, path) = match rest.split_once('/') {
        Some((host, path)) => (host.to_string(), format!("/{path}")),
        None => (rest.to_string(), "/".to_string()),
    };
    Ok((scheme, host, path))
}

/// 构造带签名的请求 URL（腾讯 HMAC-SHA1 鉴权）
///
/// ```text
/// sorted_query = 按 key 字典序拼接的「业务参数 + 签名参数」（不含 signature）
/// sign_original = "GET {host}{path}/{appid}?{sorted_query}"
/// signature = base64(HMAC-SHA1(secret_key, sign_original))
/// url = {scheme}://{host}{path}/{appid}?{sorted_query}&signature={enc}
/// ```
pub fn build_signed_url(
    base_url: &str,
    app_id: &str,
    secret_id: &str,
    secret_key: &str,
    model: &str,
    timestamp: u64,
) -> Result<String, String> {
    let (scheme, host, path) = split_ws_url(base_url)?;
    let expired = timestamp + SIGNATURE_EXPIRES_SECS;

    // 参与签名的参数（按 key 排序后拼接）
    let mut params: Vec<(&str, String)> = vec![
        ("engine_model_type", model.to_string()),
        ("expired", expired.to_string()),
        ("filter_dirty", "0".to_string()),
        ("filter_modal", "0".to_string()),
        ("filter_punc", "0".to_string()),
        ("needvad", "1".to_string()),
        ("secretid", secret_id.to_string()),
        ("timestamp", timestamp.to_string()),
        // voice_format=1：16bit PCM
        ("voice_format", "1".to_string()),
        ("word_info", "0".to_string()),
    ];
    params.sort_by_key(|(key, _)| *key);

    let sorted_query = params
        .iter()
        .map(|(key, value)| format!("{key}={}", url_encode(value)))
        .collect::<Vec<_>>()
        .join("&");

    let sign_original = format!("GET {host}{path}/{app_id}?{sorted_query}");
    let mut mac = HmacSha1::new_from_slice(secret_key.as_bytes())
        .map_err(|_| "SecretKey 含非法字符，无法用于签名".to_string())?;
    mac.update(sign_original.as_bytes());
    let signature = BASE64.encode(mac.finalize().into_bytes());

    Ok(format!(
        "{scheme}://{host}{path}/{app_id}?{sorted_query}&signature={}",
        url_encode(&signature)
    ))
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
    let code = value
        .pointer("/code")
        .and_then(|value| value.as_i64())
        .unwrap_or(0);
    if code != 0 {
        let message = value
            .pointer("/message")
            .and_then(|value| value.as_str())
            .unwrap_or("服务端未提供错误详情");
        return ServerEvent::Failed {
            code,
            message: message.to_string(),
        };
    }

    let text = value
        .pointer("/result/voice_text_str")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    if text.trim().is_empty() {
        return ServerEvent::Other;
    }
    let is_final = value
        .pointer("/result/slice_type")
        .and_then(|value| value.as_i64())
        .map(|slice| slice == 2)
        .unwrap_or(false);
    ServerEvent::Result {
        text: text.to_string(),
        is_final,
    }
}

/// 腾讯错误码 → 可操作文案
fn failed_message(code: i64, message: &str) -> String {
    let mut text = format!("腾讯云服务端返回错误（code {code}）：{message}");
    match code {
        // 签名 / 凭证错误
        4003 | 4010 | 4004 => {
            text.push_str("。请检查「设置 → 语音识别」中腾讯云的 SecretId、SecretKey 与 AppID 是否正确（访问管理「API 密钥管理」获取），并确认本机时间是否准确");
        }
        // 模型 / 参数错误
        4001 | 4005 | 4007 => {
            text.push_str("。请检查引擎模型名（默认 16k_zh）与服务端地址是否匹配");
        }
        // 配额
        4102 | 4103 => {
            text.push_str("。请到腾讯云控制台确认实时语音识别的用量与配额");
        }
        _ => {}
    }
    text
}

/// 去掉 URL 中的查询串，避免把签名写进日志
fn redact_url(url: &str) -> String {
    match url.split_once('?') {
        Some((head, _)) => format!("{head}?..."),
        None => url.to_string(),
    }
}

fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn missing_credential(field: &str) -> String {
    format!("腾讯云语音识别：尚未配置凭证字段「{field}」，请到「设置 → 语音识别」填写服务商凭证后再开始转写。")
}

/// 腾讯实时识别引擎
#[derive(Debug)]
pub struct TencentRealtimeEngine {
    engine_name: String,
    tx: Option<tokio_mpsc::Sender<WorkerMsg>>,
    results: std_mpsc::Receiver<WorkerEvent>,
    pending: Vec<CloudTranscript>,
    failure: Option<String>,
    done_rx: Option<std_mpsc::Receiver<()>>,
    handle: Option<JoinHandle<()>>,
    finished: bool,
}

impl TencentRealtimeEngine {
    /// 建立连接（阻塞；凭证 / 地址错误立即返回可操作错误）
    pub fn connect(spec: EngineSpec) -> ProviderResult<Self> {
        let app_id = spec
            .credential("app_id")
            .ok_or_else(|| ProviderError::Other(missing_credential("AppID")))?
            .to_string();
        let secret_id = spec
            .credential("secret_id")
            .ok_or_else(|| ProviderError::Other(missing_credential("SecretId")))?
            .to_string();
        let secret_key = spec
            .credential("secret_key")
            .ok_or_else(|| ProviderError::Other(missing_credential("SecretKey")))?
            .to_string();

        let base = spec.websocket_url();
        let url = build_signed_url(
            &base,
            &app_id,
            &secret_id,
            &secret_key,
            &spec.model,
            unix_now_secs(),
        )
        .map_err(ProviderError::Other)?;
        let channel = spec.channel.clone();

        let (msg_tx, msg_rx) = tokio_mpsc::channel::<WorkerMsg>(AUDIO_QUEUE);
        let (event_tx, event_rx) = std_mpsc::channel::<WorkerEvent>();
        let (ready_tx, ready_rx) = std_mpsc::channel::<Result<(), String>>();
        let (done_tx, done_rx) = std_mpsc::channel::<()>();

        let thread_url = url;
        let handle = thread::Builder::new()
            .name(format!("asr-cloud-tencent-{channel}"))
            .spawn(move || worker_main(thread_url, msg_rx, event_tx, ready_tx, done_tx))
            .map_err(|err| ProviderError::Other(format!("启动云端 ASR 线程失败：{err}")))?;

        match ready_rx.recv_timeout(Duration::from_millis(HANDSHAKE_WAIT_MS)) {
            Ok(Ok(())) => Ok(Self {
                engine_name: ENGINE_NAME.to_string(),
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
                drop(msg_tx);
                Err(ProviderError::Network(format!(
                    "连接云端 ASR 超时（{}ms），请检查网络与服务端地址：{}",
                    HANDSHAKE_WAIT_MS,
                    redact_url(&base)
                )))
            }
        }
    }

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

    fn shutdown(&mut self) {
        self.tx.take();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl CloudAsrEngine for TencentRealtimeEngine {
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
            Err(tokio_mpsc::error::TrySendError::Full(_)) => {
                tracing::debug!("云端 ASR 音频队列已满，丢弃一帧");
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

        // 1) 发送 0 字节空帧（腾讯约定的结束信号）
        if let Some(tx) = self.tx.as_ref() {
            match tx.try_send(WorkerMsg::Finish) {
                Ok(()) => {}
                Err(tokio_mpsc::error::TrySendError::Full(message)) => {
                    let _ = tx.blocking_send(message);
                }
                Err(tokio_mpsc::error::TrySendError::Closed(_)) => {}
            }
        }

        // 2) 等待服务端回最终结果并关闭（超时后强制收尾）
        if let Some(done_rx) = self.done_rx.take() {
            let _ = done_rx.recv_timeout(Duration::from_millis(FINISH_TIMEOUT_MS));
        }

        // 3) 关闭通道并回收线程
        self.shutdown();

        let mut transcripts = self.poll().unwrap_or_default();
        transcripts.extend(self.pending.drain(..));
        Ok(transcripts)
    }
}

impl Drop for TencentRealtimeEngine {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn worker_main(
    url: String,
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

    runtime.block_on(session(url, msg_rx, events, ready_tx));
    let _ = done_tx.send(());
}

/// 一次完整会话：连接（建连即识别）→ 双向循环（送音频 / 收结果）→ 空帧结束
async fn session(
    url: String,
    mut msg_rx: tokio_mpsc::Receiver<WorkerMsg>,
    events: std_mpsc::Sender<WorkerEvent>,
    ready_tx: std_mpsc::Sender<Result<(), String>>,
) {
    let mut ws = match tokio::time::timeout(
        Duration::from_millis(CONNECT_TIMEOUT_MS),
        tokio_tungstenite::connect_async(url.as_str()),
    )
    .await
    {
        Ok(Ok((ws, _))) => {
            let _ = ready_tx.send(Ok(()));
            ws
        }
        Ok(Err(err)) => {
            // 腾讯的鉴权 / 参数错误在建连阶段即以 HTTP 状态或错误 JSON 暴露
            let message = if let tokio_tungstenite::tungstenite::Error::Http(ref response) = err {
                let status = response.status().as_u16();
                if status == 401 || status == 403 {
                    format!(
                        "鉴权失败（HTTP {status}）：请检查「设置 → 语音识别」中腾讯云的 SecretId、SecretKey 与 AppID"
                    )
                } else {
                    format!("连接腾讯云 ASR 失败：HTTP {status}")
                }
            } else {
                format!("连接腾讯云 ASR 失败：{err}")
            };
            let _ = ready_tx.send(Err(message));
            return;
        }
        Err(_) => {
            let _ = ready_tx.send(Err(format!(
                "连接云端 ASR 超时（{CONNECT_TIMEOUT_MS}ms）：{}",
                redact_url(&url)
            )));
            return;
        }
    };

    let mut buffer: Vec<f32> = Vec::new();
    let mut finish_requested = false;

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
                    // 冲刷不足一帧的尾包，再发 0 字节空帧表示结束
                    if !buffer.is_empty() {
                        let tail = std::mem::take(&mut buffer);
                        let _ = ws.send(Message::binary(pcm16_le(&tail))).await;
                    }
                    if let Err(err) = ws.send(Message::binary(Vec::new())).await {
                        emit_error(&events, format!("发送结束空帧失败：{err}"));
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
                        if is_final && finish_requested {
                            return;
                        }
                    }
                    ServerEvent::Failed { code, message } => {
                        emit_error(&events, failed_message(code, &message));
                        return;
                    }
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

fn emit_error(events: &std_mpsc::Sender<WorkerEvent>, message: String) {
    let _ = events.send(WorkerEvent::Error(message));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::asr::CloudAsrProvider;
    use futures_util::StreamExt;

    fn spec(base_url: &str) -> EngineSpec {
        EngineSpec {
            provider: CloudAsrProvider::Tencent,
            base_url: base_url.to_string(),
            model: CloudAsrProvider::Tencent.default_model().to_string(),
            language: "zh-CN".to_string(),
            credentials: [
                ("app_id".to_string(), "125000123".to_string()),
                ("secret_id".to_string(), "AKIDdemo".to_string()),
                ("secret_key".to_string(), "secret-demo".to_string()),
            ]
            .into(),
            channel: "mic".to_string(),
        }
    }

    #[test]
    fn signed_url_appends_appid_and_signature() {
        let url = build_signed_url(
            "wss://asr.cloud.tencent.com/asr/v2",
            "125000123",
            "AKIDdemo",
            "secret-demo",
            "16k_zh",
            1_700_000_000,
        )
        .expect("sign");
        let (head, query) = url.split_once('?').expect("query");
        assert_eq!(head, "wss://asr.cloud.tencent.com/asr/v2/125000123");
        // 参数按 key 字典序
        let keys: Vec<&str> = query
            .split('&')
            .filter(|part| !part.starts_with("signature"))
            .map(|part| part.split('=').next().expect("key"))
            .collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted, "业务参数应按 key 排序：{query}");
        assert!(query.contains("signature="), "应带签名：{query}");
        assert!(query.contains("engine_model_type=16k_zh"));
        assert!(query.contains("voice_format=1"));
        assert!(query.contains("needvad=1"));
        assert!(query.contains("timestamp=1700000000"));
        assert!(query.contains(&format!("expired={}", 1_700_000_000 + SIGNATURE_EXPIRES_SECS)));
        // 凭证不进明文
        assert!(!query.contains("secret-demo"));
    }

    #[test]
    fn invalid_scheme_is_rejected() {
        let err = build_signed_url(
            "https://asr.example.com/v2",
            "appid",
            "sid",
            "skey",
            "16k_zh",
            0,
        )
        .expect_err("应拒绝非 WebSocket 地址");
        assert!(err.contains("ws://"));
    }

    #[test]
    fn server_event_parses_slice_type_and_errors() {
        let final_json = serde_json::json!({
            "code": 0,
            "message": "success",
            "result": { "slice_type": 2, "voice_text_str": "面试官提问" }
        })
        .to_string();
        assert_eq!(
            parse_server_event(&final_json),
            ServerEvent::Result { text: "面试官提问".into(), is_final: true }
        );

        let partial_json = serde_json::json!({
            "code": 0,
            "result": { "slice_type": 1, "voice_text_str": "面试官" }
        })
        .to_string();
        assert_eq!(
            parse_server_event(&partial_json),
            ServerEvent::Result { text: "面试官".into(), is_final: false }
        );

        let error_json = serde_json::json!({
            "code": 4003,
            "message": "signature verification error"
        })
        .to_string();
        assert_eq!(
            parse_server_event(&error_json),
            ServerEvent::Failed { code: 4003, message: "signature verification error".into() }
        );

        assert_eq!(parse_server_event("not json"), ServerEvent::Other);
    }

    #[test]
    fn error_message_gives_actionable_hint_for_credentials() {
        let message = failed_message(4003, "signature verification error");
        assert!(message.contains("SecretId"), "{message}");
        assert!(message.contains("AppID"), "{message}");
        let model_hint = failed_message(4001, "invalid parameter");
        assert!(model_hint.contains("模型"), "{model_hint}");
    }

    // ===== 本地 mock 服务端：离线端到端验证协议时序 =====

    /// 极简腾讯 mock：接受连接 → 每收 2 帧音频回 partial → 收到空帧回 final → 关闭
    async fn serve_tencent(stream: tokio::net::TcpStream, behavior: &'static str) {
        let mut ws = match tokio_tungstenite::accept_async(stream).await {
            Ok(ws) => ws,
            Err(_) => return,
        };
        if behavior == "auth_failed" {
            // 连接后立即下发签名错误 JSON。
            // 发送后短暂停留再关闭：立即 close 会让客户端的 close 处理与文本消息竞速，
            // 偶发只读到「连接已关闭」而丢掉服务端错误（并发测试下不稳定）。
            let _ = ws
                .send(Message::text(
                    serde_json::json!({ "code": 4003, "message": "signature verification error" })
                        .to_string(),
                ))
                .await;
            tokio::time::sleep(Duration::from_millis(200)).await;
            let _ = ws.close(None).await;
            return;
        }
        let mut audio_frames = 0usize;
        let mut sent_partial = false;
        while let Some(message) = ws.next().await {
            match message {
                Ok(Message::Binary(data)) => {
                    if data.is_empty() {
                        // 结束空帧：回 final 并关闭
                        let _ = ws
                            .send(Message::text(
                                serde_json::json!({
                                    "code": 0,
                                    "message": "success",
                                    "result": { "slice_type": 2, "voice_text_str": "最终结果" }
                                })
                                .to_string(),
                            ))
                            .await;
                        let _ = ws.close(None).await;
                        return;
                    }
                    audio_frames += 1;
                    if !sent_partial && audio_frames >= 2 {
                        sent_partial = true;
                        let _ = ws
                            .send(Message::text(
                                serde_json::json!({
                                    "code": 0,
                                    "result": { "slice_type": 1, "voice_text_str": "中间结果" }
                                })
                                .to_string(),
                            ))
                            .await;
                    }
                }
                Ok(Message::Close(_)) | Err(_) => return,
                _ => {}
            }
        }
    }

    fn start_mock(behavior: &'static str) -> String {
        let (port_tx, port_rx) = std_mpsc::channel::<u16>();
        thread::Builder::new()
            .name("asr-tencent-mock".to_string())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("mock runtime");
                runtime.block_on(async move {
                    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                        .await
                        .expect("bind");
                    let _ = port_tx.send(listener.local_addr().expect("addr").port());
                    loop {
                        match tokio::time::timeout(Duration::from_millis(50), listener.accept())
                            .await
                        {
                            Ok(Ok((stream, _))) => {
                                let _ = serve_tencent(stream, behavior).await;
                            }
                            Ok(Err(_)) => break,
                            Err(_) => continue,
                        }
                    }
                });
            })
            .expect("spawn");
        let port = port_rx.recv_timeout(Duration::from_secs(5)).expect("port");
        format!("ws://127.0.0.1:{port}")
    }

    fn feed_sine(engine: &mut TencentRealtimeEngine, frames: usize) {
        for i in 0..frames {
            let frame: Vec<f32> = (0..SEND_BATCH_SAMPLES)
                .map(|n| ((n + i * 13) % 20) as f32 / 10.0 - 1.0)
                .collect();
            engine.feed(&frame).expect("feed");
        }
    }

    #[test]
    fn end_to_end_round_trip_produces_partial_then_final() {
        let url = start_mock("normal");
        let mut engine = TencentRealtimeEngine::connect(spec(&url)).expect("connect");
        assert_eq!(engine.name(), "tencent-realtime");

        feed_sine(&mut engine, 8);
        // 轮询等待 partial（并发测试下时序余量）
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let partials = loop {
            let drained = engine.drain().expect("drain");
            if drained.iter().any(|t| !t.is_final) {
                break drained;
            }
            if std::time::Instant::now() >= deadline {
                panic!("5s 内未收到 partial 结果");
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        assert!(
            partials.iter().any(|t| !t.is_final),
            "应收到 partial：{partials:?}"
        );

        let finals = engine.finish().expect("finish");
        assert!(
            finals.iter().any(|t| t.is_final && t.text.contains("最终结果")),
            "finish 应收到 final：{finals:?}"
        );
    }

    #[test]
    fn auth_failure_returns_actionable_error() {
        let url = start_mock("auth_failed");
        let mut engine = TencentRealtimeEngine::connect(spec(&url)).expect("connect");
        feed_sine(&mut engine, 1);
        // 轮询等待错误上报（并发测试下时序余量）
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let error = loop {
            match engine.drain() {
                Err(err) => break err.to_string(),
                Ok(_) if std::time::Instant::now() >= deadline => {
                    panic!("鉴权错误未在 5s 内上报")
                }
                Ok(_) => std::thread::sleep(Duration::from_millis(50)),
            }
        };
        assert!(error.contains("4003"), "{error}");
        assert!(error.contains("SecretId"), "应给出可操作提示：{error}");
    }

    #[test]
    fn connect_fails_fast_on_unreachable_address() {
        let mut bad = spec("ws://127.0.0.1:1");
        bad.credentials.insert("app_id".into(), "x".into());
        let err = TencentRealtimeEngine::connect(bad)
            .expect_err("不可达地址应连接失败")
            .to_string();
        assert!(err.contains("失败") || err.contains("超时"), "{err}");
    }
}
