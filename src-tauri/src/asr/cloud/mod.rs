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
pub mod openai_compat;
pub mod tencent;
pub mod xfyun;

use crate::config::asr::CloudAsrProvider;
use crate::providers::types::ProviderResult;
use std::collections::BTreeMap;

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
    /// 服务商凭证（字段因服务商而异：阿里云/OpenAI 兼容 `api_key`，讯飞
    /// `app_id`+`api_key`+`api_secret`，腾讯 `app_id`+`secret_id`+`secret_key`）
    pub credentials: BTreeMap<String, String>,
    /// 通道标识（`mic` / `system`），用于线程名与日志区分
    pub channel: String,
}

impl EngineSpec {
    /// 实时识别使用的 WebSocket 地址：`http(s)://` 自动改写为 `ws(s)://`
    pub fn websocket_url(&self) -> String {
        crate::config::asr::to_websocket_url(&self.base_url)
    }

    /// 读取非空凭证字段
    pub fn credential(&self, field: &str) -> Option<&str> {
        self.credentials
            .get(field)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
    }

    /// 测试构造：单凭证（阿里云 / OpenAI 兼容）
    #[cfg(test)]
    pub fn with_api_key(provider: CloudAsrProvider, base_url: &str, api_key: &str) -> Self {
        Self {
            provider,
            base_url: base_url.to_string(),
            model: provider.default_model().to_string(),
            language: "zh-CN".to_string(),
            credentials: [("api_key".to_string(), api_key.to_string())].into(),
            channel: "mic".to_string(),
        }
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
        CloudAsrProvider::Xfyun => Ok(Box::new(xfyun::XfyunRealtimeEngine::connect(spec)?)),
        CloudAsrProvider::Tencent => Ok(Box::new(tencent::TencentRealtimeEngine::connect(spec)?)),
        CloudAsrProvider::OpenaiCompat => Ok(Box::new(openai_compat::OpenAiCompatEngine::connect(spec)?)),
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
            credentials: [("api_key".to_string(), "secret".to_string())].into(),
            channel: "mic".into(),
        };
        assert_eq!(spec.websocket_url(), "wss://proxy.example.com/ws");
    }

    #[test]
    fn credential_lookup_ignores_blank_values() {
        let mut spec = EngineSpec::with_api_key(CloudAsrProvider::Aliyun, "wss://x.example.com/ws", "k");
        assert_eq!(spec.credential("api_key"), Some("k"));
        spec.credentials.insert("api_secret".into(), "   ".into());
        assert_eq!(spec.credential("api_secret"), None, "空白凭证应视为缺失");
    }

    #[test]
    fn every_provider_routes_to_a_protocol_engine() {
        // TODO-10 / TODO-11 落地后，四家服务商在工厂层都有协议分支；
        // 「未接入」拦截由 `implemented()` 收口（现为全量 true），此测试锁定工厂不再
        // 返回 unsupported_provider_message。
        for provider in crate::config::asr::all_cloud_providers() {
            let spec = EngineSpec {
                provider,
                base_url: "ws://127.0.0.1:1/unused".into(),
                model: "m".into(),
                language: "zh-CN".into(),
                credentials: provider
                    .credential_fields()
                    .iter()
                    .map(|field| (field.to_string(), "x".to_string()))
                    .collect(),
                channel: "test".into(),
            };
            // 连接必然失败（不可达地址），但错误**不应**是「待接入」文案
            let message = match connect_cloud_engine(spec) {
                Ok(_) => panic!("{provider:?} 连接不应成功"),
                Err(err) => err.to_string(),
            };
            assert!(
                !message.contains("待接入"),
                "{provider:?} 已接入协议，不应再返回待接入错误：{message}"
            );
        }
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
