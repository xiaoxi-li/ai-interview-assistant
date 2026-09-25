//! ASR Provider：多服务商（讯飞/阿里/腾讯）与本地（sherpa-onnx）统一抽象。

use super::types::{
    AsrMode, AudioChunk, ProviderError, ProviderResult, Speaker, TranscriptEvent,
};
use std::sync::mpsc::{self, Receiver, Sender};

/// 实时语音识别抽象
pub trait AsrProvider: Send {
    /// Provider 标识（如 xfyun / sherpa-onnx / mock）
    fn name(&self) -> &str;

    /// 运行模式（云端 / 本地）
    fn mode(&self) -> AsrMode;

    /// 喂入一帧音频
    fn feed(&mut self, chunk: AudioChunk) -> ProviderResult<()>;

    /// 结束本次流式会话
    fn finish(&mut self) -> ProviderResult<()>;

    /// 获取转写事件流（每个实例仅可获取一次）
    fn take_events(&mut self) -> ProviderResult<Receiver<TranscriptEvent>>;
}

/// 离线开发与测试用的 Mock ASR：按帧累计文本，结束时产出 final
pub struct MockAsrProvider {
    mode: AsrMode,
    speaker: Speaker,
    tx: Sender<TranscriptEvent>,
    rx: Option<Receiver<TranscriptEvent>>,
    partial: String,
}

impl MockAsrProvider {
    pub fn new(mode: AsrMode, speaker: Speaker) -> Self {
        let (tx, rx) = mpsc::channel();
        Self { mode, speaker, tx, rx: Some(rx), partial: String::new() }
    }
}

impl AsrProvider for MockAsrProvider {
    fn name(&self) -> &str {
        "mock"
    }

    fn mode(&self) -> AsrMode {
        self.mode
    }

    fn feed(&mut self, chunk: AudioChunk) -> ProviderResult<()> {
        self.partial
            .push_str(&format!("[{}B]", chunk.data.len()));
        self.tx
            .send(TranscriptEvent::Partial {
                speaker: self.speaker,
                text: self.partial.clone(),
            })
            .map_err(|e| ProviderError::Other(e.to_string()))
    }

    fn finish(&mut self) -> ProviderResult<()> {
        let text = std::mem::take(&mut self.partial);
        self.tx
            .send(TranscriptEvent::Final { speaker: self.speaker, text })
            .map_err(|e| ProviderError::Other(e.to_string()))
    }

    fn take_events(&mut self) -> ProviderResult<Receiver<TranscriptEvent>> {
        self.rx
            .take()
            .ok_or_else(|| ProviderError::Other("转写事件流已被获取".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(len: usize) -> AudioChunk {
        AudioChunk {
            data: vec![0u8; len],
            sample_rate: 16_000,
            channels: 1,
            ts: 0,
        }
    }

    #[test]
    fn mock_emits_partial_then_final() {
        let mut asr = MockAsrProvider::new(AsrMode::Local, Speaker::Interviewer);
        let rx = asr.take_events().expect("events");

        asr.feed(chunk(320)).expect("feed #1");
        asr.feed(chunk(160)).expect("feed #2");
        asr.finish().expect("finish");

        // Mock 在同线程内同步产出事件，直接取用缓冲即可。
        // 注意不能使用 rx.iter()：发送端仍由 provider 持有，通道不会关闭，会永久阻塞。
        let events: Vec<TranscriptEvent> = rx.try_iter().collect();
        assert_eq!(events.len(), 3);
        assert!(matches!(
            &events[0],
            TranscriptEvent::Partial { speaker: Speaker::Interviewer, text } if text == "[320B]"
        ));
        assert!(matches!(
            &events[1],
            TranscriptEvent::Partial { text, .. } if text == "[320B][160B]"
        ));
        assert!(matches!(
            &events[2],
            TranscriptEvent::Final { text, .. } if text == "[320B][160B]"
        ));
    }

    #[test]
    fn events_can_only_be_taken_once() {
        let mut asr = MockAsrProvider::new(AsrMode::Cloud, Speaker::Me);
        assert!(asr.take_events().is_ok());
        assert!(asr.take_events().is_err());
    }

    #[test]
    fn mode_reflects_construction() {
        let cloud = MockAsrProvider::new(AsrMode::Cloud, Speaker::Me);
        let local = MockAsrProvider::new(AsrMode::Local, Speaker::Me);
        assert_eq!(cloud.mode(), AsrMode::Cloud);
        assert_eq!(local.mode(), AsrMode::Local);
    }
}
