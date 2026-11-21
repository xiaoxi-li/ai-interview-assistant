//! OpenAI 兼容（自定义）HTTP 转写客户端（IA-16.9 / TODO-11）。
//!
//! 语义适配：`/v1/audio/transcriptions` 是**请求/响应式**接口（整段音频一次上传），
//! 与 WebSocket 持续双向流式不同。本引擎的映射策略：
//!
//! * `feed()`：缓冲 16kHz f32 音频，在采集泵线程上做**轻量分段**（帧级能量静音
//!   检测 + 最大段长上限），满足条件的段交给专职线程（`std::thread`）；
//! * 专职线程：把段封装为 WAV，`multipart/form-data` POST 到
//!   `{Base URL}/audio/transcriptions`，响应文本作为一条 `Final` 结果回传；
//! * `drain()`：非阻塞拉取已完成段的识别结果；`finish()`：冲刷尾段并等待回收线程。
//!   因此本通路**没有 partial 实时结果**，句子粒度取决于分段（静音 800ms 截断 /
//!   15s 上限），适合「面试官问完一句再出结果」的节奏。
//! * `connect()`：发送一段受控的最小测试音频验证端点与鉴权（`连接测试`不会假阳性）。
//!
//! 双通道 = 两个引擎实例各自独立分段与上传，互不共享状态。

use super::{CloudAsrEngine, CloudTranscript, EngineSpec};
use crate::providers::types::{ProviderError, ProviderResult};
use std::sync::mpsc as std_mpsc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// 引擎标识（`asr_status.engine` 展示）
pub const ENGINE_NAME: &str = "openai-compat";

/// 段上传整体超时（含网络与服务端处理；connect 探测复用同一超时）
const UPLOAD_TIMEOUT_MS: u64 = 30_000;
/// 段上传队列容量（满则丢段，避免阻塞采集线程）
const SEGMENT_QUEUE: usize = 8;
/// 尾部静音截断阈值：静音达到此时长即认为一句话结束
const SILENCE_SPLIT_MS: u32 = 800;
/// 段内最少语音时长（过短的静音段直接丢弃）
const MIN_VOICED_MS: u32 = 600;
/// 段长上限（防止单段无限增长；Whisper 接口对单请求有大小/时长限制）
const MAX_SEGMENT_MS: u32 = 15_000;
/// 帧长（20ms @ 16kHz）
const FRAME_MS: u32 = 20;
/// 静音判定：帧 RMS 低于该阈值视为静音
const SILENCE_RMS: f32 = 0.008;
/// `finish()` 等待专职线程退出的超时（限时等待，避免「停止转写」长时间阻塞；
/// 超时后剩余上传线程由 Drop 兜底回收）
const FINISH_JOIN_TIMEOUT_MS: u64 = 10_000;

/// HTTP 状态码
const HTTP_UNAUTHORIZED: u16 = 401;
const HTTP_FORBIDDEN: u16 = 403;
const HTTP_NOT_FOUND: u16 = 404;

/// 工作线程指令
enum WorkerMsg {
    /// 一个完整的语音段（f32 样本）
    Segment(Vec<f32>),
}

/// 工作线程回传事件
enum WorkerEvent {
    Transcript(CloudTranscript),
    Error(String),
}

/// OpenAI 兼容 HTTP 转写引擎
#[derive(Debug)]
pub struct OpenAiCompatEngine {
    engine_name: String,
    tx: Option<std_mpsc::SyncSender<WorkerMsg>>,
    results: std_mpsc::Receiver<WorkerEvent>,
    failure: Option<String>,
    handle: Option<JoinHandle<()>>,
    /// 当前段缓冲（f32 样本）
    buffer: Vec<f32>,
    /// 缓冲总时长（ms）
    buffer_ms: u32,
    /// 缓冲末尾连续静音时长（ms）
    silence_tail_ms: u32,
    finished: bool,
}

impl OpenAiCompatEngine {
    /// 建连：对端点发送一段受控的最小测试音频，验证地址与鉴权
    pub fn connect(spec: EngineSpec) -> ProviderResult<Self> {
        let api_key = spec
            .credential("api_key")
            .ok_or_else(|| {
                ProviderError::Other(
                    "OpenAI 兼容（自定义）：尚未配置 API Key，请到「设置 → 语音识别」填写服务商凭证后再开始转写。"
                        .to_string(),
                )
            })?
            .to_string();

        let endpoint = transcriptions_endpoint(&spec.base_url)
            .map_err(ProviderError::Other)?;
        let model = spec.model.trim().to_string();
        if model.is_empty() {
            return Err(ProviderError::Other(
                "云端模型名不能为空（OpenAI 兼容默认 whisper-1）".into(),
            ));
        }
        let language = language_code(&spec.language);

        // 发送 0.5s 静音测试音频：2xx = 端点与鉴权可用（连接测试不产生假阳性）
        let probe = vec![0.0f32; 8_000];
        let wav = wav_bytes(&probe);
        let (boundary, body) = multipart_body(&wav, &model, language.as_deref());
        upload_segment(&endpoint, &api_key, &boundary, &body).map_err(|err| {
            ProviderError::Network(format!(
                "OpenAI 兼容端点不可用：{err}（地址：{endpoint}）"
            ))
        })?;

        let (tx, rx) = std_mpsc::sync_channel::<WorkerMsg>(SEGMENT_QUEUE);
        let (event_tx, event_rx) = std_mpsc::channel::<WorkerEvent>();
        let thread_endpoint = endpoint.clone();
        let thread_model = model.clone();
        let thread_api_key = api_key;
        let thread_language = language.clone();
        let channel = spec.channel.clone();

        let handle = thread::Builder::new()
            .name(format!("asr-cloud-openai-{channel}"))
            .spawn(move || {
                while let Ok(message) = rx.recv() {
                    match message {
                        WorkerMsg::Segment(samples) => {
                            if let Err(message) = upload_and_emit(
                                &thread_endpoint,
                                &thread_api_key,
                                &thread_model,
                                thread_language.as_deref(),
                                &samples,
                                &event_tx,
                            ) {
                                // 上传失败属于致命错误：上报一次并停止继续投递
                                let _ = event_tx.send(WorkerEvent::Error(message));
                                return;
                            }
                        }
                    }
                }
            })
            .map_err(|err| {
                ProviderError::Other(format!("启动云端 ASR 线程失败：{err}"))
            })?;

        Ok(Self {
            engine_name: ENGINE_NAME.to_string(),
            tx: Some(tx),
            results: event_rx,
            failure: None,
            handle: Some(handle),
            buffer: Vec::new(),
            buffer_ms: 0,
            silence_tail_ms: 0,
            finished: false,
        })
    }

    /// 分段触发条件：末尾静音足够长且段内语音足够多，或达到段长上限
    fn should_flush(&self) -> bool {
        let voiced_ms = self.buffer_ms.saturating_sub(self.silence_tail_ms);
        (self.silence_tail_ms >= SILENCE_SPLIT_MS && voiced_ms >= MIN_VOICED_MS)
            || self.buffer_ms >= MAX_SEGMENT_MS
    }

    /// 截断当前段：去掉尾部静音；整段静音则丢弃
    fn take_segment(&mut self) -> Option<Vec<f32>> {
        let silence_samples = (self.silence_tail_ms / FRAME_MS) as usize
            * crate::audio::FRAME_SAMPLES;
        let voiced_samples = self.buffer.len().saturating_sub(silence_samples);
        let segment: Vec<f32> = self.buffer.drain(..voiced_samples).collect();
        self.buffer.clear();
        self.buffer_ms = 0;
        self.silence_tail_ms = 0;
        let voiced_ms = (segment.len() / crate::audio::FRAME_SAMPLES) as u32 * FRAME_MS;
        if voiced_ms < MIN_VOICED_MS {
            return None;
        }
        Some(segment)
    }

    /// 把当前段投递给专职线程（队列满则丢弃整段）
    fn submit_segment(&mut self) {
        if let Some(segment) = self.take_segment() {
            if let Some(tx) = self.tx.as_ref() {
                if tx.try_send(WorkerMsg::Segment(segment)).is_err() {
                    tracing::debug!("OpenAI 兼容段队列已满，丢弃一段音频");
                }
            }
        }
    }

    fn poll(&mut self) -> ProviderResult<Vec<CloudTranscript>> {
        let mut transcripts = Vec::new();
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
}

impl CloudAsrEngine for OpenAiCompatEngine {
    fn name(&self) -> &str {
        &self.engine_name
    }

    fn feed(&mut self, frame: &[f32]) -> ProviderResult<()> {
        if frame.is_empty() || self.finished || self.tx.is_none() {
            return Ok(());
        }

        self.buffer.extend_from_slice(frame);
        self.buffer_ms += FRAME_MS;
        let rms = crate::audio::capture::rms_level(frame);
        if rms >= SILENCE_RMS {
            self.silence_tail_ms = 0;
        } else {
            self.silence_tail_ms += FRAME_MS;
        }

        if self.should_flush() {
            self.submit_segment();
        }
        Ok(())
    }

    fn drain(&mut self) -> ProviderResult<Vec<CloudTranscript>> {
        self.poll()
    }

    fn finish(&mut self) -> ProviderResult<Vec<CloudTranscript>> {
        if self.finished {
            return self.poll();
        }
        self.finished = true;

        // 1) 冲刷尾段（有语音就发，纯静音直接丢）
        self.submit_segment();

        // 2) 通知专职线程收尾（处理完队列即可退出），限时等待（不阻塞停止转写太久）
        self.tx.take();
        if let Some(handle) = self.handle.take() {
            let deadline =
                std::time::Instant::now() + Duration::from_millis(FINISH_JOIN_TIMEOUT_MS);
            while !handle.is_finished() && std::time::Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
            if handle.is_finished() {
                let _ = handle.join();
            } else {
                // 线程仍在处理最后一段上传：不再等待，Drop 时兜底回收
                self.handle = Some(handle);
                tracing::warn!("OpenAI 兼容引擎收尾超时，剩余上传线程将由 Drop 兜底回收");
            }
        }

        // 3) 拉取全部剩余结果
        let transcripts = self.poll().unwrap_or_default();
        if transcripts.is_empty() {
            if let Some(message) = self.failure.take() {
                return Err(ProviderError::Network(message));
            }
        }
        Ok(transcripts)
    }
}

impl Drop for OpenAiCompatEngine {
    fn drop(&mut self) {
        // 关闭投递通道 → 专职线程 recv 返回 Err 并退出
        self.tx.take();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// 上传一个语音段并把文本作为 Final 结果回传（由持有凭证的专职线程闭包执行）
///
/// 注：凭证通过线程闭包捕获，不经过任何日志或全局状态。
fn upload_and_emit(
    endpoint: &str,
    api_key: &str,
    model: &str,
    language: Option<&str>,
    samples: &[f32],
    events: &std_mpsc::Sender<WorkerEvent>,
) -> Result<(), String> {
    let wav = wav_bytes(samples);
    let (boundary, body) = multipart_body(&wav, model, language);
    let text = upload_segment(endpoint, api_key, &boundary, &body)?;
    let trimmed = text.trim();
    if !trimmed.is_empty() {
        let _ = events.send(WorkerEvent::Transcript(CloudTranscript {
            text: trimmed.to_string(),
            is_final: true,
        }));
    }
    Ok(())
}

/// 拼接转写端点：`{Base URL}/audio/transcriptions`（容忍末尾斜杠）
///
/// Base URL 需包含版本路径（官方默认 `https://api.openai.com/v1`）。
pub fn transcriptions_endpoint(base_url: &str) -> Result<String, String> {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("OpenAI 兼容的服务端地址（Base URL）不能为空".into());
    }
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return Err(format!(
            "OpenAI 兼容的服务端地址应以 http:// 或 https:// 开头：{trimmed}"
        ));
    }
    Ok(format!("{trimmed}/audio/transcriptions"))
}

/// `zh-CN` → ISO-639-1 `zh`；空值返回 `None`
fn language_code(language: &str) -> Option<String> {
    let trimmed = language.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.split('-').next().unwrap_or(trimmed).to_lowercase())
}

/// f32 → WAV（单声道 16bit PCM 16kHz）字节流
pub fn wav_bytes(samples: &[f32]) -> Vec<u8> {
    let pcm = pcm16_le(samples);
    let data_len = pcm.len() as u32;
    let sample_rate: u32 = 16_000;
    let byte_rate = sample_rate * 2; // 单声道 * 2 字节

    let mut out = Vec::with_capacity(44 + pcm.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // fmt 块长度
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // 单声道
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes()); // 块对齐
    out.extend_from_slice(&16u16.to_le_bytes()); // 位深
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.extend_from_slice(&pcm);
    out
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

/// 手工构造 `multipart/form-data` 请求体（返回 boundary 与 body）
///
/// 字段：`file`（audio.wav）+ `model` + 可选 `language`。不引入额外 HTTP multipart 依赖。
pub fn multipart_body(wav: &[u8], model: &str, language: Option<&str>) -> (String, Vec<u8>) {
    let boundary = "----EasyInterviewASR7MA4YWxkTrZu0gW";
    let mut body = Vec::new();

    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"audio.wav\"\r\nContent-Type: audio/wav\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(wav);
    body.extend_from_slice(
        format!("\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"model\"\r\n\r\n{model}\r\n")
            .as_bytes(),
    );
    if let Some(language) = language {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"language\"\r\n\r\n{language}\r\n"
            )
            .as_bytes(),
        );
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    (boundary.to_string(), body)
}

/// 执行一次转写上传，返回响应文本（2xx）；失败返回可操作错误
fn upload_segment(
    endpoint: &str,
    api_key: &str,
    boundary: &str,
    body: &[u8],
) -> Result<String, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_millis(UPLOAD_TIMEOUT_MS))
        .build();
    let response = agent
        .post(endpoint)
        .set("Authorization", &format!("Bearer {api_key}"))
        .set(
            "Content-Type",
            &format!("multipart/form-data; boundary={boundary}"),
        )
        .send_bytes(body);

    match response {
        Ok(response) => {
            let status = response.status();
            let text = response
                .into_string()
                .map_err(|err| format!("读取响应失败：{err}"))?;
            parse_success(status, &text)
        }
        Err(ureq::Error::Status(code, response)) => {
            let text = response.into_string().unwrap_or_default();
            Err(map_http_error(code, &text, endpoint))
        }
        Err(err) => Err(format!("请求失败：{err}")),
    }
}

/// 2xx 响应解析：`{"text": "..."}`
fn parse_success(status: u16, body: &str) -> Result<String, String> {
    debug_assert!((200..300).contains(&status));
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        // 某些兼容实现返回纯文本
        return Ok(body.trim().to_string());
    };
    match value.get("text").and_then(|value| value.as_str()) {
        Some(text) => Ok(text.to_string()),
        None => Err(format!(
            "服务端返回了无法解析的响应（HTTP {status}）：{}",
            truncate(body, 200)
        )),
    }
}

/// HTTP 错误映射：带上状态码与 `error.message`
fn map_http_error(code: u16, body: &str, endpoint: &str) -> String {
    let detail = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|value| {
            value
                .pointer("/error/message")
                .and_then(|value| value.as_str())
                .map(|value| value.to_string())
                .or_else(|| {
                    value
                        .get("message")
                        .and_then(|value| value.as_str())
                        .map(|value| value.to_string())
                })
        })
        .unwrap_or_else(|| truncate(body, 200));

    match code {
        HTTP_UNAUTHORIZED | HTTP_FORBIDDEN => format!(
            "鉴权失败（HTTP {code}）：请检查「设置 → 语音识别」中的 API Key 是否正确、是否已过期"
        ),
        HTTP_NOT_FOUND => format!(
            "服务端返回 HTTP 404（地址：{endpoint}）：请确认 Base URL 包含版本路径（例如 https://api.openai.com/v1），且该服务商兼容 /audio/transcriptions 接口"
        ),
        _ => format!("转写请求失败（HTTP {code}）：{detail}"),
    }
}

fn truncate(value: &str, max: usize) -> String {
    if value.len() <= max {
        value.to_string()
    } else {
        format!("{}…", &value[..max])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::asr::CloudAsrProvider;

    fn spec(base_url: &str) -> EngineSpec {
        EngineSpec {
            provider: CloudAsrProvider::OpenaiCompat,
            base_url: base_url.to_string(),
            model: CloudAsrProvider::OpenaiCompat.default_model().to_string(),
            language: "zh-CN".to_string(),
            credentials: [("api_key".to_string(), "sk-test".to_string())].into(),
            channel: "mic".to_string(),
        }
    }

    #[test]
    fn endpoint_joins_path_and_tolerates_trailing_slash() {
        assert_eq!(
            transcriptions_endpoint("https://api.openai.com/v1").expect("endpoint"),
            "https://api.openai.com/v1/audio/transcriptions"
        );
        assert_eq!(
            transcriptions_endpoint("https://asr.example.com/v1/").expect("endpoint"),
            "https://asr.example.com/v1/audio/transcriptions"
        );
        let err = transcriptions_endpoint("wss://asr.example.com").expect_err("应拒绝 ws 地址");
        assert!(err.contains("http"), "{err}");
    }

    #[test]
    fn language_maps_to_iso_639_1() {
        assert_eq!(language_code("zh-CN").as_deref(), Some("zh"));
        assert_eq!(language_code("en").as_deref(), Some("en"));
        assert_eq!(language_code("  ").as_deref(), None);
    }

    #[test]
    fn wav_header_matches_expected_layout() {
        let samples: Vec<f32> = (0..16_000).map(|i| (i as f32 / 100.0).sin()).collect();
        let wav = wav_bytes(&samples);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        // 采样率 16000（字节 24-27）
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 16_000);
        // 单声道
        assert_eq!(u16::from_le_bytes(wav[22..24].try_into().unwrap()), 1);
        // 16bit
        assert_eq!(u16::from_le_bytes(wav[34..36].try_into().unwrap()), 16);
        assert_eq!(&wav[36..40], b"data");
        // data 长度 = 样本数 * 2
        assert_eq!(
            u32::from_le_bytes(wav[40..44].try_into().unwrap()),
            (16_000 * 2) as u32
        );
        assert_eq!(wav.len(), 44 + 16_000 * 2);
    }

    #[test]
    fn multipart_body_contains_fields_and_boundary() {
        let wav = wav_bytes(&[0.5; 100]);
        let (boundary, body) = multipart_body(&wav, "whisper-1", Some("zh"));
        let text = String::from_utf8_lossy(&body).to_string();
        assert!(text.contains(&format!("--{boundary}")));
        assert!(text.contains("name=\"file\"; filename=\"audio.wav\""));
        assert!(text.contains("Content-Type: audio/wav"));
        assert!(text.contains("name=\"model\""));
        assert!(text.contains("whisper-1"));
        assert!(text.contains("name=\"language\""));
        assert!(text.contains("zh"));
        assert!(text.ends_with(&format!("--{boundary}--\r\n")));

        let (_, no_lang) = multipart_body(&wav, "whisper-1", None);
        assert!(!String::from_utf8_lossy(&no_lang).contains("language"));
    }

    #[test]
    fn success_response_parses_text_field() {
        assert_eq!(
            parse_success(200, r#"{"text":"你好"}"#).expect("parse"),
            "你好"
        );
        // 纯文本响应（部分兼容实现）
        assert_eq!(parse_success(200, "你好").expect("parse"), "你好");
        assert!(parse_success(200, r#"{"unexpected":true}"#).is_err());
    }

    #[test]
    fn http_errors_map_to_actionable_hints() {
        let unauthorized = map_http_error(401, r#"{"error":{"message":"bad key"}}"#, "https://x/v1/audio/transcriptions");
        assert!(unauthorized.contains("API Key"), "{unauthorized}");

        let not_found = map_http_error(404, "", "https://x/v1/audio/transcriptions");
        assert!(not_found.contains("Base URL"), "{not_found}");

        let generic = map_http_error(500, r#"{"error":{"message":"boom"}}"#, "https://x/v1/audio/transcriptions");
        assert!(generic.contains("boom"), "{generic}");
    }

    #[test]
    fn long_error_bodies_are_truncated() {
        let long = "x".repeat(500);
        // "…" 在 UTF-8 中占 3 字节：200 个 ASCII 字符 + 3 字节省略号
        assert_eq!(truncate(&long, 200).len(), 203);
        assert_eq!(truncate("short", 200), "short");
    }

    // ===== 本地 HTTP mock：离线端到端验证（connect 探测 / 分段上传 / 错误映射） =====

    /// 极简 HTTP mock：解析请求（含 Authorization 与 body），按 `mode` 回复
    fn serve_request(stream: &mut std::net::TcpStream, mode: &str) {
        use std::io::{Read, Write};
        // 读 headers
        let mut buffer = [0u8; 1];
        let mut request = Vec::new();
        while request.windows(4).any(|w| w == b"\r\n\r\n") == false {
            match stream.read(&mut buffer) {
                Ok(0) => return,
                Ok(_) => request.push(buffer[0]),
                Err(_) => return,
            }
            if request.len() > 16_384 {
                return;
            }
        }
        let head = String::from_utf8_lossy(&request).to_string();
        let content_length: usize = head
            .lines()
            .find_map(|line| {
                let lower = line.to_lowercase();
                if lower.starts_with("content-length:") {
                    lower["content-length:".len()..].trim().parse().ok()
                } else {
                    None
                }
            })
            .unwrap_or(0);
        // 读 body
        let mut body = vec![0u8; content_length];
        let mut read = 0;
        while read < content_length {
            match stream.read(&mut body[read..]) {
                Ok(0) => break,
                Ok(n) => read += n,
                Err(_) => return,
            }
        }

        let is_authorized = head.contains("Authorization: Bearer sk-test");
        let response = if mode == "unauthorized" {
            (
                "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                r#"{"error":{"message":"Incorrect API key"}}"#.to_string(),
            )
        } else if !is_authorized {
            (
                "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                r#"{"error":{"message":"no auth header"}}"#.to_string(),
            )
        } else if mode == "ok_text" {
            (
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                r#"{"text":"你好面试官"}"#.to_string(),
            )
        } else if mode == "not_found" {
            (
                "HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                r#"{"error":{"message":"no route"}}"#.to_string(),
            )
        } else {
            // echo：把收到的 multipart 字段回显，验证请求形状
            let body_text = String::from_utf8_lossy(&body).to_string();
            let model_ok = body_text.contains("whisper-1");
            let wav_ok = body.windows(4).any(|w| w == b"RIFF");
            let payload = format!(
                r#"{{"text":"model_ok={model_ok} wav_ok={wav_ok}"}}"#
            );
            (
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                payload,
            )
        };
        let (_, payload) = &response;
        let head = response
            .0
            .replacen("{}", &payload.len().to_string(), 1);
        let _ = stream.write_all(head.as_bytes());
        let _ = stream.write_all(payload.as_bytes());
        let _ = stream.flush();
    }

    struct HttpMock {
        url: String,
        _stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    }

    fn start_http_mock(mode: &'static str) -> HttpMock {
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop_handle = stop.clone();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let url = format!("http://127.0.0.1:{}", listener.local_addr().expect("addr").port());
        thread::Builder::new()
            .name("asr-openai-mock".to_string())
            .spawn(move || {
                for stream in listener.incoming() {
                    if stop_handle.load(std::sync::atomic::Ordering::SeqCst) {
                        break;
                    }
                    match stream {
                        Ok(mut stream) => serve_request(&mut stream, mode),
                        Err(_) => break,
                    }
                }
            })
            .expect("spawn");
        HttpMock { url, _stop: stop }
    }

    /// 生成「1s 语音 + 1s 静音」两段，模拟采集泵的 20ms 帧
    fn speech_and_silence_frames() -> Vec<Vec<f32>> {
        let speech = vec![0.5f32; crate::audio::FRAME_SAMPLES];
        let silence = vec![0.0f32; crate::audio::FRAME_SAMPLES];
        let mut frames = Vec::new();
        for _ in 0..50 {
            frames.push(speech.clone());
        }
        for _ in 0..50 {
            frames.push(silence.clone());
        }
        frames
    }

    #[test]
    fn connect_probe_fails_on_unauthorized() {
        let mock = start_http_mock("unauthorized");
        let mut bad = spec(&mock.url);
        bad.credentials.insert("api_key".into(), "sk-test".into());
        let err = OpenAiCompatEngine::connect(bad)
            .expect_err("鉴权失败应报错")
            .to_string();
        assert!(err.contains("API Key"), "{err}");
        assert!(err.contains("端点不可用"), "{err}");
    }

    #[test]
    fn connect_probe_fails_on_not_found() {
        let mock = start_http_mock("not_found");
        let err = OpenAiCompatEngine::connect(spec(&mock.url))
            .expect_err("404 应报错")
            .to_string();
        assert!(err.contains("Base URL"), "{err}");
    }

    #[test]
    fn end_to_end_segments_are_uploaded_and_transcribed() {
        let mock = start_http_mock("echo");
        let mut engine = OpenAiCompatEngine::connect(spec(&mock.url)).expect("connect");
        assert_eq!(engine.name(), "openai-compat");

        // 喂「语音 + 静音」：静音满足 800ms 阈值后触发一次分段上传
        for frame in speech_and_silence_frames() {
            engine.feed(&frame).expect("feed");
        }
        // 等待专职线程完成上传
        std::thread::sleep(Duration::from_millis(300));
        let transcripts = engine.drain().expect("drain");
        assert!(!transcripts.is_empty(), "应收到分段结果");
        assert!(transcripts.iter().all(|t| t.is_final), "HTTP 通路只有 final");
        assert!(
            transcripts
                .iter()
                .any(|t| t.text.contains("model_ok=true") && t.text.contains("wav_ok=true")),
            "mock 应回显模型与 WAV 校验结果：{transcripts:?}"
        );

        // finish：冲刷 + 回收线程
        let finals = engine.finish().expect("finish");
        assert!(
            finals.iter().all(|t| t.is_final),
            "HTTP 通路全部为 final：{finals:?}"
        );
    }

    #[test]
    fn pure_silence_is_never_uploaded() {
        let mock = start_http_mock("echo");
        let mut engine = OpenAiCompatEngine::connect(spec(&mock.url)).expect("connect");
        for _ in 0..100 {
            engine.feed(&[0.0; crate::audio::FRAME_SAMPLES]).expect("feed");
        }
        std::thread::sleep(Duration::from_millis(200));
        let transcripts = engine.drain().expect("drain");
        assert!(transcripts.is_empty(), "纯静音不应产生结果：{transcripts:?}");
    }

    #[test]
    fn missing_api_key_is_rejected_before_any_request() {
        let mock = start_http_mock("ok_text");
        let mut no_key = spec(&mock.url);
        no_key.credentials.remove("api_key");
        let err = OpenAiCompatEngine::connect(no_key)
            .expect_err("缺 Key 应报错")
            .to_string();
        assert!(err.contains("API Key"), "{err}");
    }

    #[test]
    fn feed_after_finish_is_ignored() {
        let mock = start_http_mock("ok_text");
        let mut engine = OpenAiCompatEngine::connect(spec(&mock.url)).expect("connect");
        engine.finish().expect("finish");
        // 幂等：finish 后再 feed / finish 不应出错
        engine.feed(&[0.5; crate::audio::FRAME_SAMPLES]).expect("feed after finish");
        engine.finish().expect("second finish");
    }
}
