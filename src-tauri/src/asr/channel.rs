//! 单通道转写出口抽象（IA-16）。
//!
//! 本地是「VAD 分段 + 段级识别」（[`AsrPipeline`]），云端是「持续双向流式」
//! （[`CloudAsrEngine`]）——两者语义不同，但对上层（采集泵 / 双通道会话 / 事件广播 /
//! 落库 / 问题识别）只需要同一个出口：[`AsrChannel`]。
//!
//! * `impl AsrChannel for AsrPipeline`：纯委托，本地行为保持完全不变；
//! * [`CloudChannel`]：把云端引擎适配成同一出口——`push_frame` 送帧并拉取结果，
//!   `flush` 结束任务并冲刷末尾结果。一旦发生致命错误（鉴权失败 / 连接中断），
//!   只上报**一次**错误事件并停止后续投递，避免逐帧刷屏。

use super::cloud::{CloudAsrEngine, CloudTranscript};
use super::pipeline::AsrPipeline;
use crate::providers::types::{Speaker, TranscriptEvent};

/// 单通道转写出口：送帧返回事件，`flush` 结束并冲刷
pub trait AsrChannel: Send {
    /// 通道/引擎标识（供界面与日志）
    fn name(&self) -> &str;

    /// 送入一帧 16kHz 单声道 f32 音频，返回本次产生的事件
    fn push_frame(&mut self, frame: &[f32], ts: i64) -> Vec<TranscriptEvent>;

    /// 音频流结束：结束任务并冲刷末尾结果（幂等）
    fn flush(&mut self) -> Vec<TranscriptEvent>;
}

impl AsrChannel for AsrPipeline {
    fn name(&self) -> &str {
        self.engine_name()
    }

    fn push_frame(&mut self, frame: &[f32], ts: i64) -> Vec<TranscriptEvent> {
        AsrPipeline::push_frame(self, frame, ts)
    }

    fn flush(&mut self) -> Vec<TranscriptEvent> {
        AsrPipeline::flush(self)
    }
}

/// 云端通道：持续把音频喂给云端引擎，并把句级结果映射为 partial / final
pub struct CloudChannel {
    engine: Box<dyn CloudAsrEngine>,
    speaker: Speaker,
    /// 连接已失效：只保留一次错误上报，不再投递（避免逐帧刷屏）
    failed: bool,
}

impl CloudChannel {
    pub fn new(engine: Box<dyn CloudAsrEngine>, speaker: Speaker) -> Self {
        Self {
            engine,
            speaker,
            failed: false,
        }
    }

    /// 句级结果 → 转写事件（`is_final` 决定落库与否）
    fn map_results(&self, transcripts: Vec<CloudTranscript>) -> Vec<TranscriptEvent> {
        transcripts
            .into_iter()
            .filter(|transcript| !transcript.text.trim().is_empty())
            .map(|transcript| {
                if transcript.is_final {
                    TranscriptEvent::Final {
                        speaker: self.speaker,
                        text: transcript.text,
                    }
                } else {
                    TranscriptEvent::Partial {
                        speaker: self.speaker,
                        text: transcript.text,
                    }
                }
            })
            .collect()
    }

    /// 致命错误处理：上报一次并停止后续投递
    fn fail(&mut self, message: String, events: &mut Vec<TranscriptEvent>) {
        if self.failed {
            return;
        }
        self.failed = true;
        events.push(TranscriptEvent::Error { message });
    }
}

impl AsrChannel for CloudChannel {
    fn name(&self) -> &str {
        self.engine.name()
    }

    fn push_frame(&mut self, frame: &[f32], _ts: i64) -> Vec<TranscriptEvent> {
        if self.failed {
            return Vec::new();
        }
        let mut events = Vec::new();
        match self.engine.feed(frame) {
            Ok(()) => {
                match self.engine.drain() {
                    Ok(transcripts) => events.extend(self.map_results(transcripts)),
                    Err(err) => self.fail(err.to_string(), &mut events),
                }
            }
            Err(err) => self.fail(err.to_string(), &mut events),
        }
        events
    }

    fn flush(&mut self) -> Vec<TranscriptEvent> {
        if self.failed {
            return Vec::new();
        }
        match self.engine.finish() {
            Ok(transcripts) => self.map_results(transcripts),
            Err(err) => {
                let mut events = Vec::new();
                self.fail(err.to_string(), &mut events);
                events
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asr::engine::ReplayEngine;
    use crate::audio::FRAME_SAMPLES;
    use crate::providers::types::{ProviderError, ProviderResult};

    fn speech_frame() -> Vec<f32> {
        (0..FRAME_SAMPLES)
            .map(|index| if index % 2 == 0 { 0.8 } else { -0.8 })
            .collect()
    }

    fn silence_frame() -> Vec<f32> {
        vec![0.0; FRAME_SAMPLES]
    }

    #[test]
    fn pipeline_channel_delegates_to_local_pipeline() {
        let pipeline = AsrPipeline::with_defaults(
            Box::new(ReplayEngine::with_script(vec!["本地通道".into()])),
            Speaker::Interviewer,
            0.5,
        );
        let mut channel: Box<dyn AsrChannel> = Box::new(pipeline);
        assert_eq!(channel.name(), "replay");

        let mut events = Vec::new();
        for _ in 0..30 {
            events.extend(channel.push_frame(&speech_frame(), 0));
        }
        for _ in 0..30 {
            events.extend(channel.push_frame(&silence_frame(), 0));
        }

        assert!(events.iter().any(|event| matches!(
            event,
            TranscriptEvent::Final { text, .. } if text == "本地通道"
        )));
    }

    /// 测试替身：按脚本返回结果，可注入致命错误
    struct FakeCloudEngine {
        script: Vec<CloudTranscript>,
        failed: bool,
    }

    impl CloudAsrEngine for FakeCloudEngine {
        fn name(&self) -> &str {
            "fake-cloud"
        }

        fn feed(&mut self, _frame: &[f32]) -> ProviderResult<()> {
            if self.failed {
                return Err(ProviderError::Network("连接已断开".into()));
            }
            Ok(())
        }

        fn drain(&mut self) -> ProviderResult<Vec<CloudTranscript>> {
            Ok(std::mem::take(&mut self.script))
        }

        fn finish(&mut self) -> ProviderResult<Vec<CloudTranscript>> {
            Ok(Vec::new())
        }
    }

    #[test]
    fn cloud_channel_maps_partial_and_final_with_speaker() {
        let engine = FakeCloudEngine {
            script: vec![
                CloudTranscript {
                    text: "面试官在问".into(),
                    is_final: false,
                },
                CloudTranscript {
                    text: "面试官在问问题".into(),
                    is_final: true,
                },
                CloudTranscript {
                    text: "   ".into(),
                    is_final: true,
                },
            ],
            failed: false,
        };
        let mut channel = CloudChannel::new(Box::new(engine), Speaker::Interviewer);
        assert_eq!(channel.name(), "fake-cloud");

        let events = channel.push_frame(&speech_frame(), 0);
        assert_eq!(
            events,
            vec![
                TranscriptEvent::Partial {
                    speaker: Speaker::Interviewer,
                    text: "面试官在问".into()
                },
                TranscriptEvent::Final {
                    speaker: Speaker::Interviewer,
                    text: "面试官在问问题".into()
                },
            ],
            "空白文本应被过滤"
        );
        assert!(channel.flush().is_empty());
    }

    #[test]
    fn cloud_channel_reports_fatal_error_once_and_stops() {
        let engine = FakeCloudEngine {
            script: Vec::new(),
            failed: true,
        };
        let mut channel = CloudChannel::new(Box::new(engine), Speaker::Me);

        let first = channel.push_frame(&speech_frame(), 0);
        assert_eq!(first.len(), 1);
        assert!(matches!(
            &first[0],
            TranscriptEvent::Error { message } if message.contains("连接已断开")
        ));

        // 之后不再重复刷屏
        assert!(channel.push_frame(&speech_frame(), 0).is_empty());
        assert!(channel.flush().is_empty(), "已失效的通道 flush 不应再报错");
    }
}
