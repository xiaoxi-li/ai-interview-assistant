//! 讯飞开放平台（v2/iat）实时语音识别 WebSocket 客户端（IA-16.9 / TODO-10）。
//!
//! 协议要点（对照官方《语音听写流式 WebAPI》文档）：
//! * 地址：默认 `wss://ws-api.xfyun.cn/v2/iat`，可由设置页 Base URL 覆盖；
//! * 鉴权：HMAC-SHA256 签名拼入 URL 查询串（`authorization` / `date` / `host`），
//!   凭证为三元组 `app_id` + `api_key` + `api_secret`；
//! * 指令：首帧 JSON（`common` + `business` + `data.status=0`）→ 中间帧（`data.status=1`）
//!   → 尾帧（`data.status=2`），音频以 base64 raw PCM16 附在 `data.audio`；
//! * 结果：`data.data.result.ws[].cw[].w` 拼接为文本，`data.data.status==2` 表示句结束；
//! * 音频：单声道 16bit PCM、16kHz，40ms（1280 样本）一帧；
//! * 连接建好后进入双向循环，上层线程只做非阻塞投递与拉取（同阿里云线程模型）。

use super::{CloudAsrEngine, CloudTranscript, EngineSpec};
use crate::providers::types::{ProviderError, ProviderResult};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::sync::mpsc as std_mpsc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc as tokio_mpsc;
use tokio_tungstenite::tungstenite::Message;

/// 引擎标识（`asr_status.engine` 展示）
pub const ENGINE_NAME: &str = "xfyun-iat";

/// 连接 + 握手超时（工作线程内部）
const CONNECT_TIMEOUT_MS: u64 = 8_000;
/// 主线程等待握手结果的超时（略大于内部超时）
const HANDSHAKE_WAIT_MS: u64 = 10_000;
/// `finish()` 等待服务端收尾的超时
const FINISH_TIMEOUT_MS: u64 = 3_000;
/// 音频聚合目标：1280 样本 = 40ms @16kHz（讯飞推荐帧长）
const SEND_BATCH_SAMPLES: usize = 1_280;
/// 音频投递队列容量（满则丢帧而不是阻塞采集线程）
const AUDIO_QUEUE: usize = 128;

type HmacSha256 = Hmac<Sha256>;

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
    /// 识别结果：`status == 2` 表示句结束（final）
    Result { text: String, is_final: bool },
    /// `code != 0`（鉴权 / 参数 / 配额等错误）
    Failed { code: i64, message: String },
    /// 其他 / 无法解析
    Other,
}

/// RFC 1123 HTTP 日期（GMT），用于讯飞签名的 `date` 参数
pub fn http_date_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let secs_of_day = secs.rem_euclid(86_400);
    let (hour, minute, second) = (
        secs_of_day / 3_600,
        (secs_of_day % 3_600) / 60,
        secs_of_day % 60,
    );

    // civil_from_days（Howard Hinnant 算法）
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year_of_era = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year_of_era + 1 } else { year_of_era };

    const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    // 1970-01-01 是周四
    let weekday = (days + 4).rem_euclid(7) as usize;

    format!(
        "{}, {:02} {} {} {:02}:{:02}:{:02} GMT",
        WEEKDAYS[weekday], day, MONTHS[(month - 1) as usize], year, hour, minute, second
    )
}

/// 仅 URL 查询串场景的百分号编码（保留字母数字与 `-_.~`）
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

/// 从 `ws(s)://host/path` 地址解析 `(scheme, host, path)`
fn split_ws_url(url: &str) -> Result<(String, String, String), String> {
    let (scheme, rest) = url
        .strip_prefix("wss://")
        .map(|rest| ("wss".to_string(), rest))
        .or_else(|| url.strip_prefix("ws://").map(|rest| ("ws".to_string(), rest)))
        .ok_or_else(|| {
            format!("服务端地址不是合法的 WebSocket 地址：{url}（应以 ws:// 或 wss:// 开头）")
        })?;
    let (host, path) = match rest.split_once('/') {
        Some((host, path)) => (host.to_string(), format!("/{path}")),
        None => (rest.to_string(), "/".to_string()),
    };
    Ok((scheme, host, path))
}

/// 构造带签名的请求 URL（讯飞 HMAC-SHA256 鉴权）
///
/// ```text
/// signature_origin = "host: {host}\ndate: {date}\nGET {path} HTTP/1.1"
/// signature = base64(HMAC-SHA256(api_secret, signature_origin))
/// authorization_origin = api_key="...", algorithm="hmac-sha256",
///     headers="host date request-line", signature="..."
/// authorization = base64(authorization_origin)
/// url = {scheme}://{host}{path}?authorization={enc}&date={enc}&host={host}
/// ```
pub fn build_signed_url(
    url: &str,
    api_key: &str,
    api_secret: &str,
    date: &str,
) -> Result<String, String> {
    let (scheme, host, path) = split_ws_url(url)?;
    let signature_origin = format!("host: {host}\ndate: {date}\nGET {path} HTTP/1.1");

    let mut mac = HmacSha256::new_from_slice(api_secret.as_bytes())
        .map_err(|_| "API Secret 含非法字符，无法用于签名".to_string())?;
    mac.update(signature_origin.as_bytes());
    let signature = BASE64.encode(mac.finalize().into_bytes());

    let authorization_origin = format!(
        "api_key=\"{api_key}\", algorithm=\"hmac-sha256\", headers=\"host date request-line\", signature=\"{signature}\""
    );
    let authorization = BASE64.encode(authorization_origin);

    Ok(format!(
        "{scheme}://{host}{path}?authorization={}&date={}&host={}",
        url_encode(&authorization),
        url_encode(date),
        url_encode(&host)
    ))
}

/// 首帧（`status = 0`，携带 `common` / `business`）
pub fn build_first_frame(app_id: &str, language: &str, audio_b64: &str) -> String {
    serde_json::json!({
        "common": { "app_id": app_id },
        "business": {
            "language": to_xfyun_language(language),
            "domain": "iat",
            "accent": "mandarin",
        },
        "data": {
            "status": 0,
            "format": "audio/L16;rate=16000",
            "encoding": "raw",
            "audio": audio_b64
        }
    })
    .to_string()
}

/// 中间帧（`status = 1`）与尾帧（`status = 2`，`audio` 允许为空）
pub fn build_data_frame(status: u8, audio_b64: &str) -> String {
    serde_json::json!({
        "data": {
            "status": status,
            "format": "audio/L16;rate=16000",
            "encoding": "raw",
            "audio": audio_b64
        }
    })
    .to_string()
}

/// `zh-CN` → `zh_cn`（讯飞语言取值）；未知值原样返回（支持用户直接填 `en_us` 等）
fn to_xfyun_language(language: &str) -> String {
    let normalized = language.trim().replace('-', "_").to_lowercase();
    match normalized.as_str() {
        "zh_cn" | "zh_tw" | "en_us" => normalized,
        "zh" => "zh_cn".to_string(),
        "en" => "en_us".to_string(),
        _ => language.trim().to_string(),
    }
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
    let code = value.pointer("/code").and_then(|value| value.as_i64()).unwrap_or(0);
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

    let status = value
        .pointer("/data/data/status")
        .and_then(|value| value.as_i64())
        .unwrap_or(-1);
    if status < 0 {
        return ServerEvent::Other;
    }

    // 拼接 result.ws[].cw[].w
    let mut text = String::new();
    if let Some(words) = value.pointer("/data/data/result/ws").and_then(|v| v.as_array()) {
        for word in words {
            if let Some(candidates) = word.get("cw").and_then(|v| v.as_array()) {
                for candidate in candidates {
                    if let Some(part) = candidate.get("w").and_then(|v| v.as_str()) {
                        text.push_str(part);
                    }
                }
            }
        }
    }
    if text.is_empty() {
        return ServerEvent::Other;
    }
    ServerEvent::Result {
        text,
        is_final: status == 2,
    }
}

/// 讯飞错误码 → 可操作文案
fn failed_message(code: i64, message: &str) -> String {
    let mut text = format!("讯飞服务端返回错误（code {code}）：{message}");
    match code {
        // appid / api_key / api_secret 相关
        10105 | 10106 | 10107 | 10165 => {
            text.push_str("。请检查「设置 → 语音识别」中讯飞的 AppID、API Key 与 API Secret 是否正确（控制台「语音听写」页面获取）");
        }
        // 授权到期 / 授权不足
        10005 | 10019 => {
            text.push_str("。请到讯飞控制台确认语音听写服务的授权与用量");
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

/// 讯飞实时识别引擎
pub struct XfyunRealtimeEngine {
    engine_name: String,
    tx: Option<tokio_mpsc::Sender<WorkerMsg>>,
    results: std_mpsc::Receiver<WorkerEvent>,
    pending: Vec<CloudTranscript>,
    failure: Option<String>,
    done_rx: Option<std_mpsc::Receiver<()>>,
    handle: Option<JoinHandle<()>>,
    finished: bool,
}

impl XfyunRealtimeEngine {
    /// 建立连接（阻塞；凭证 / 地址错误立即返回可操作错误）
    pub fn connect(spec: EngineSpec) -> ProviderResult<Self> {
        let app_id = spec
            .credential("app_id")
            .ok_or_else(|| ProviderError::Other(missing_credential("AppID")))?
            .to_string();
        let api_key = spec
            .credential("api_key")
            .ok_or_else(|| ProviderError::Other(missing_credential("API Key")))?
            .to_string();
        let api_secret = spec
            .credential("api_secret")
            .ok_or_else(|| ProviderError::Other(missing_credential("API Secret")))?
            .to_string();

        let base = spec.websocket_url();
        let url = build_signed_url(&base, &api_key, &api_secret, &http_date_now())
            .map_err(ProviderError::Other)?;
        let first_frame_prelude = serde_json::json!({
            "common": { "app_id": app_id },
            "business": {
                "language": to_xfyun_language(&spec.language),
                "domain": "iat",
                "accent": "mandarin",
            }
        });
        let channel = spec.channel.clone();

        let (msg_tx, msg_rx) = tokio_mpsc::channel::<WorkerMsg>(AUDIO_QUEUE);
        let (event_tx, event_rx) = std_mpsc::channel::<WorkerEvent>();
        let (ready_tx, ready_rx) = std_mpsc::channel::<Result<(), String>>();
        let (done_tx, done_rx) = std_mpsc::channel::<()>();

        let thread_url = url.clone();
        let handle = thread::Builder::new()
            .name(format!("asr-cloud-xfyun-{channel}"))
            .spawn(move || {
                worker_main(
                    thread_url, first_frame_prelude, msg_rx, event_tx, ready_tx, done_tx,
                )
            })
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

fn missing_credential(field: &str) -> String {
    format!("讯飞开放平台：尚未配置凭证字段「{field}」，请到「设置 → 语音识别」填写服务商凭证后再开始转写。")
}

impl CloudAsrEngine for XfyunRealtimeEngine {
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

        // 1) 请求结束任务（发尾帧 status=2）
        if let Some(tx) = self.tx.as_ref() {
            match tx.try_send(WorkerMsg::Finish) {
                Ok(()) => {}
                Err(tokio_mpsc::error::TrySendError::Full(message)) => {
                    let _ = tx.blocking_send(message);
                }
                Err(tokio_mpsc::error::TrySendError::Closed(_)) => {}
            }
        }

        // 2) 等待服务端回 final / 关闭（超时后强制收尾）
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

impl Drop for XfyunRealtimeEngine {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// 工作线程入口
fn worker_main(
    url: String,
    first_frame_prelude: serde_json::Value,
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

    runtime.block_on(session(
        url,
        first_frame_prelude,
        msg_rx,
        events,
        ready_tx,
    ));
    let _ = done_tx.send(());
}

/// 一次完整会话：连接 → 首帧 → 双向循环（送音频 / 收结果）→ 尾帧
async fn session(
    url: String,
    first_frame_prelude: serde_json::Value,
    mut msg_rx: tokio_mpsc::Receiver<WorkerMsg>,
    events: std_mpsc::Sender<WorkerEvent>,
    ready_tx: std_mpsc::Sender<Result<(), String>>,
) {
    // 1) 连接（讯飞握手即建连成功，鉴权错误由首个响应 JSON 的 code 暴露）
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
            let _ = ready_tx.send(Err(format!("连接讯飞云端 ASR 失败：{err}")));
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

    // 2) 双向循环
    let mut buffer: Vec<f32> = Vec::new();
    let mut first_frame_sent = false;
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
                        let frame = frame_for(&first_frame_prelude, &mut first_frame_sent, &batch);
                        if let Err(err) = ws.send(Message::text(frame)).await {
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
                    let tail = std::mem::take(&mut buffer);
                    let frame = frame_for(&first_frame_prelude, &mut first_frame_sent, &tail);
                    let last = serde_json::from_str::<serde_json::Value>(&frame)
                        .ok()
                        .and_then(|mut value| {
                            value["data"]["status"] = serde_json::json!(2);
                            Some(value.to_string())
                        })
                        .unwrap_or_else(|| build_data_frame(2, ""));
                    if let Err(err) = ws.send(Message::text(last)).await {
                        emit_error(&events, format!("发送尾帧失败：{err}"));
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
                            // 尾帧后的 final：直接收尾
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

/// 首帧带 `common` / `business`，其后只有 `data`
fn frame_for(
    prelude: &serde_json::Value,
    first_frame_sent: &mut bool,
    samples: &[f32],
) -> String {
    let audio_b64 = BASE64.encode(pcm16_le(samples));
    if *first_frame_sent {
        build_data_frame(1, &audio_b64)
    } else {
        *first_frame_sent = true;
        let mut frame = prelude.clone();
        frame["data"] = serde_json::json!({
            "status": 0,
            "format": "audio/L16;rate=16000",
            "encoding": "raw",
            "audio": audio_b64
        });
        frame.to_string()
    }
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
            provider: CloudAsrProvider::Xfyun,
            base_url: base_url.to_string(),
            model: CloudAsrProvider::Xfyun.default_model().to_string(),
            language: "zh-CN".to_string(),
            credentials: [
                ("app_id".to_string(), "app-123".to_string()),
                ("api_key".to_string(), "key-abc".to_string()),
                ("api_secret".to_string(), "secret-xyz".to_string()),
            ]
            .into(),
            channel: "mic".to_string(),
        }
    }

    #[test]
    fn signed_url_contains_encoded_query() {
        let url = build_signed_url(
            "wss://ws-api.xfyun.cn/v2/iat",
            "key-abc",
            "secret-xyz",
            "Tue, 21 Nov 2026 06:00:00 GMT",
        )
        .expect("sign");
        let (head, query) = url.split_once('?').expect("query");
        assert_eq!(head, "wss://ws-api.xfyun.cn/v2/iat");
        assert!(query.contains("authorization="), "应带 authorization：{query}");
        assert!(query.contains("date=Tue%2C%2021%20Nov%202026%2006%3A00%3A00%20GMT"));
        assert!(query.contains("host=ws-api.xfyun.cn"));
        // 签名参数不进明文
        assert!(!query.contains("secret-xyz"));
        assert!(!query.contains("key-abc"));
    }

    #[test]
    fn http_date_format_matches_rfc1123() {
        let date = http_date_now();
        // "Tue, 21 Nov 2026 06:00:00 GMT" 形状校验
        let parts: Vec<&str> = date.split(' ').collect();
        assert_eq!(parts.len(), 6, "{date}");
        assert!(parts[0].ends_with(','));
        assert_eq!(parts[5], "GMT");
        assert!(parts[3].len() == 4, "年份应为四位：{date}");
    }

    #[test]
    fn first_frame_carries_credentials_and_business() {
        let frame = build_first_frame("app-1", "zh-CN", "aGVsbG8=");
        let value: serde_json::Value = serde_json::from_str(&frame).expect("json");
        assert_eq!(value["common"]["app_id"], "app-1");
        assert_eq!(value["business"]["language"], "zh_cn");
        assert_eq!(value["business"]["domain"], "iat");
        assert_eq!(value["data"]["status"], 0);
        assert_eq!(value["data"]["format"], "audio/L16;rate=16000");
    }

    #[test]
    fn language_mapping_covers_common_forms() {
        assert_eq!(to_xfyun_language("zh-CN"), "zh_cn");
        assert_eq!(to_xfyun_language("zh"), "zh_cn");
        assert_eq!(to_xfyun_language("en"), "en_us");
        assert_eq!(to_xfyun_language("en_us"), "en_us");
    }

    #[test]
    fn server_event_parses_result_and_errors() {
        let final_json = serde_json::json!({
            "code": 0,
            "data": {
                "data": {
                    "status": 2,
                    "result": {
                        "ws": [
                            { "cw": [{ "w": "面" }] },
                            { "cw": [{ "w": "试官" }] }
                        ]
                    }
                }
            }
        })
        .to_string();
        assert_eq!(
            parse_server_event(&final_json),
            ServerEvent::Result { text: "面试官".into(), is_final: true }
        );

        let partial_json = serde_json::json!({
            "code": 0,
            "data": { "data": { "status": 1, "result": { "ws": [ { "cw": [{ "w": "你" }] } ] } } }
        })
        .to_string();
        assert_eq!(
            parse_server_event(&partial_json),
            ServerEvent::Result { text: "你".into(), is_final: false }
        );

        let error_json = serde_json::json!({
            "code": 10105,
            "message": "invalid api key"
        })
        .to_string();
        assert_eq!(
            parse_server_event(&error_json),
            ServerEvent::Failed { code: 10105, message: "invalid api key".into() }
        );

        assert_eq!(parse_server_event("not json"), ServerEvent::Other);
    }

    #[test]
    fn error_message_gives_actionable_hint_for_credentials() {
        let message = failed_message(10105, "invalid api key");
        assert!(message.contains("AppID"), "{message}");
        assert!(message.contains("API Secret"), "{message}");
        let generic = failed_message(10800, "internal error");
        assert!(generic.contains("10800"));
    }

    // ===== 本地 mock 服务端：离线端到端验证协议时序 =====

    /// 极简讯飞 mock：接受连接 → 首帧后回 partial → 收到尾帧回 final → 关闭
    async fn serve_xfyun(
        stream: tokio::net::TcpStream,
        behavior: &str,
    ) {
        let mut ws = match tokio_tungstenite::accept_async(stream).await {
            Ok(ws) => ws,
            Err(_) => return,
        };
        let mut got_first = false;
        let mut audio_frames = 0usize;
        while let Some(message) = ws.next().await {
            match message {
                Ok(Message::Text(text)) => {
                    let value: serde_json::Value = match serde_json::from_str(&text) {
                        Ok(value) => value,
                        Err(_) => continue,
                    };
                    let status = value["data"]["status"].as_i64().unwrap_or(-1);
                    if status == 0 {
                        got_first = true;
                        if behavior == "auth_failed" {
                            let _ = ws
                                .send(Message::text(
                                    serde_json::json!({
                                        "code": 10105,
                                        "message": "api key mismatch"
                                    })
                                    .to_string(),
                                ))
                                .await;
                            // 不主动关闭：等对端读到错误再退出，避免错误帧未送达就断连
                            while let Some(message) = ws.next().await {
                                match message {
                                    Ok(Message::Close(_)) | Err(_) => return,
                                    _ => {}
                                }
                            }
                            return;
                        }
                    } else if status == 2 {
                        // 尾帧：回 final 并关闭
                        let _ = ws
                            .send(Message::text(
                                serde_json::json!({
                                    "code": 0,
                                    "data": { "data": { "status": 2, "result": {
                                        "ws": [ { "cw": [ { "w": "最终结果" } ] } ]
                                    } } }
                                })
                                .to_string(),
                            ))
                            .await;
                        let _ = ws.close(None).await;
                        return;
                    }
                    // 含音频的帧计数（讯飞音频以 base64 附在 JSON 文本帧里）
                    if value["data"]["audio"]
                        .as_str()
                        .map(|audio| !audio.is_empty())
                        .unwrap_or(false)
                    {
                        audio_frames += 1;
                    }
                }
                Ok(Message::Close(_)) | Err(_) => return,
                _ => {}
            }
            // 音频攒够后回一次 partial（模拟中间结果）
            if got_first && audio_frames >= 2 {
                audio_frames = 0;
                let _ = ws
                    .send(Message::text(
                        serde_json::json!({
                            "code": 0,
                            "data": { "data": { "status": 1, "result": {
                                "ws": [ { "cw": [ { "w": "中间" } ] } ]
                            } } }
                        })
                        .to_string(),
                    ))
                    .await;
            }
        }
    }

    fn start_mock(behavior: &'static str) -> String {
        let (port_tx, port_rx) = std_mpsc::channel::<u16>();
        thread::Builder::new()
            .name("asr-xfyun-mock".to_string())
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
                                let _ = serve_xfyun(stream, behavior).await;
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

    /// 喂 40ms 一帧的正弦音频，模拟采集泵
    fn feed_sine(engine: &mut XfyunRealtimeEngine, frames: usize) {
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
        let mut engine = XfyunRealtimeEngine::connect(spec(&url)).expect("connect");
        assert_eq!(engine.name(), "xfyun-iat");

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
            "应收到 partial 结果：{partials:?}"
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
        let result = XfyunRealtimeEngine::connect(spec(&url));
        // 连接本身成功（讯飞鉴权错误在首个 JSON 响应中），错误在 drain 时上报
        let mut engine = result.expect("connect");
        feed_sine(&mut engine, 2);
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
        assert!(error.contains("10105"), "{error}");
        assert!(error.contains("AppID"), "应给出可操作提示：{error}");
    }
}
