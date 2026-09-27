//! 云端 ASR 通路（IA-16）：引擎抽象、连接规格与工厂。
//!
//! 设计要点：
//! * 云端识别是**持续双向流式**（服务端自行做端点检测），与本地「VAD 分段 + 段级识别」
//!   语义不同，因此单独抽象 [`CloudAsrEngine`]，再由
//!   [`CloudChannel`](crate::asr::channel::CloudChannel) 适配成与本地一致的通道出口。
//! * 引擎独占**专职线程 + 单线程 tokio runtime**：`tokio::select!` 在「待发送音频」与
//!   「WS 读流」之间公平调度，避免同步 WS 依赖 socket 读超时的脆弱实现。
//! * 协议适配按服务商逐个落地；未接入者由 [`connect_cloud_engine`] 返回**可操作错误**。
//! * 日志与错误文案**不含凭证明文**（只出现服务商名与地址）。

pub mod aliyun;
#[cfg(test)]
pub mod mock;

use crate::config::asr::{unsupported_provider_message, CloudAsrProvider};
use crate::providers::types::{ProviderError, ProviderResult};

/// 云端识别结果（句级）：`is_final = true` 表示该句已结束（落库）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudTranscript {
    pub text: String,
    pub is_final: bool,
}

/// 建立云端引擎所需的规格（凭证不参与日志输出）
#[derive(Debug, Clone)]
pub struct EngineSpec {
    pub provider: CloudAsrProvider,
    /// 生效的服务端地址（已完成「自定义 → 官方默认」回退）
    pub base_url: String,
    pub model: String,
    pub language: String,
    /// 服务商凭证（阿里云为 API Key）
    pub api_key: String,
    /// 通道标识（`mic` / `system`），用于线程名与日志区分
    pub channel: String,
}

impl EngineSpec {
    /// 实时识别使用的 WebSocket 地址：`http(s)://` 自动改写为 `ws(s)://`
    pub fn websocket_url(&self) -> String {
        crate::config::asr::to_websocket_url(&self.base_url)
    }
}

/// 云端流式识别引擎
pub trait CloudAsrEngine: Send {
    /// 引擎标识（供界面与日志）
    fn name(&self) -> &str;

    /// 送入一帧 16kHz 单声道 f32 音频（内部转 PCM16）
    fn feed(&mut self, frame: &[f32]) -> ProviderResult<()>;

    /// 非阻塞拉取自上次调用以来的识别结果
    fn drain(&mut self) -> ProviderResult<Vec<CloudTranscript>>;

    /// 结束任务并冲刷末尾结果（幂等，带超时）
    fn finish(&mut self) -> ProviderResult<Vec<CloudTranscript>>;
}

/// 按服务商建立云端引擎（连接与握手在内部同步完成，失败立即返回可操作错误）
pub fn connect_cloud_engine(spec: EngineSpec) -> ProviderResult<Box<dyn CloudAsrEngine>> {
    match spec.provider {
        CloudAsrProvider::Aliyun => Ok(Box::new(aliyun::AliyunRealtimeEngine::connect(spec)?)),
        other => Err(ProviderError::Other(unsupported_provider_message(other))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn websocket_url_rewrites_http_scheme() {
        let spec = EngineSpec {
            provider: CloudAsrProvider::Aliyun,
            base_url: "https://proxy.example.com/ws".into(),
            model: "gummy-realtime-v1".into(),
            language: "zh-CN".into(),
            api_key: "secret".into(),
            channel: "mic".into(),
        };
        assert_eq!(spec.websocket_url(), "wss://proxy.example.com/ws");
    }

    #[test]
    fn unimplemented_provider_returns_actionable_error() {
        let spec = EngineSpec {
            provider: CloudAsrProvider::Tencent,
            base_url: CloudAsrProvider::Tencent.default_base_url().into(),
            model: "16k_zh".into(),
            language: "zh-CN".into(),
            api_key: "secret".into(),
            channel: "mic".into(),
        };
        let message = match connect_cloud_engine(spec) {
            Ok(_) => panic!("腾讯云尚未接入，应返回错误"),
            Err(err) => err.to_string(),
        };
        assert!(message.contains("待接入"), "{message}");
        assert!(message.contains("阿里云"), "应指出当前可用服务商：{message}");
        assert!(message.contains("设置"), "应给出可操作引导：{message}");
    }

    #[test]
    fn cloud_transcript_marks_final_sentences() {
        let transcript = CloudTranscript {
            text: "你好".into(),
            is_final: true,
        };
        assert!(transcript.is_final);
    }
}
