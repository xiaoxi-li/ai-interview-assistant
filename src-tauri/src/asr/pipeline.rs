//! 本地 ASR 流水线：VAD 分段 → 引擎识别 → partial / final 事件。
//!
//! 事件语义：
//! * `Partial`：段内中间结果，会随音频增长被覆盖（仅用于界面实时展示，不落库）；
//! * `Final`：一个语音段结束后的最终结果（落库，保证崩溃不丢数据）；
//! * `Error`：引擎错误（如模型未下载 / 推理未接入），相同错误会被去重，避免刷屏。

use super::engine::LocalAsrEngine;
use crate::audio::capture::FRAME_MS;
use crate::audio::vad::{VadConfig, VadSegmenter};
use crate::audio::FRAME_SAMPLES;
use crate::providers::types::{Speaker, TranscriptEvent};

/// 默认 partial 刷新间隔（毫秒）
pub const DEFAULT_PARTIAL_INTERVAL_MS: u32 = 800;

/// 一帧对应的毫秒数（供测试与调用方参考）
pub const FRAME_DURATION_MS: u32 = FRAME_MS;

/// 本地 ASR 流水线
pub struct AsrPipeline {
    segmenter: VadSegmenter,
    engine: Box<dyn LocalAsrEngine>,
    speaker: Speaker,
    partial_interval_frames: u32,
    frames_since_partial: u32,
    last_partial: String,
    last_error: Option<String>,
}

impl AsrPipeline {
    pub fn new(
        engine: Box<dyn LocalAsrEngine>,
        speaker: Speaker,
        vad_config: VadConfig,
        vad_threshold: f32,
        partial_interval_ms: u32,
    ) -> Self {
        let interval_frames = if partial_interval_ms == 0 {
            1
        } else {
            (partial_interval_ms.div_ceil(FRAME_MS)).max(1)
        };

        Self {
            segmenter: VadSegmenter::with_energy_vad(vad_config, vad_threshold),
            engine,
            speaker,
            partial_interval_frames: interval_frames,
            frames_since_partial: 0,
            last_partial: String::new(),
            last_error: None,
        }
    }

    /// 使用默认 VAD 阈值与 partial 间隔构造
    pub fn with_defaults(engine: Box<dyn LocalAsrEngine>, speaker: Speaker, vad_threshold: f32) -> Self {
        Self::new(
            engine,
            speaker,
            VadConfig::default(),
            vad_threshold,
            DEFAULT_PARTIAL_INTERVAL_MS,
        )
    }

    pub fn engine_name(&self) -> &str {
        self.engine.name()
    }

    pub fn speaker(&self) -> Speaker {
        self.speaker
    }

    /// 送入一帧 16kHz 单声道 f32 音频
    pub fn push_frame(&mut self, frame: &[f32], ts: i64) -> Vec<TranscriptEvent> {
        let mut events = Vec::new();
        if frame.len() < FRAME_SAMPLES {
            return events;
        }

        // 1) 完整语音段 → final
        if let Some(segment) = self.segmenter.push_frame(frame, ts) {
            self.frames_since_partial = 0;
            self.last_partial.clear();
            match self.engine.transcribe_segment(&segment.samples, true) {
                Ok(Some(text)) if !text.trim().is_empty() => {
                    self.last_error = None;
                    events.push(TranscriptEvent::Final {
                        speaker: self.speaker,
                        text,
                    });
                }
                Ok(_) => {}
                Err(err) => self.push_error(&mut events, err.to_string()),
            }
            return events;
        }

        // 2) 段内 partial（仅在已确认的语音段中产出）
        self.frames_since_partial += 1;
        if !self.segmenter.is_in_confirmed_speech()
            || self.frames_since_partial < self.partial_interval_frames
        {
            return events;
        }
        self.frames_since_partial = 0;

        let current = self.segmenter.current_segment_samples();
        if current.is_empty() {
            return events;
        }

        match self.engine.transcribe_segment(&current.to_vec(), false) {
            Ok(Some(text)) if !text.trim().is_empty() && text != self.last_partial => {
                self.last_error = None;
                self.last_partial = text.clone();
                events.push(TranscriptEvent::Partial {
                    speaker: self.speaker,
                    text,
                });
            }
            Ok(_) => {}
            Err(err) => self.push_error(&mut events, err.to_string()),
        }
        events
    }

    /// 音频流结束：吐出未完成的语音段与引擎尾部结果
    pub fn flush(&mut self) -> Vec<TranscriptEvent> {
        let mut events = Vec::new();

        if let Some(segment) = self.segmenter.flush() {
            match self.engine.transcribe_segment(&segment.samples, true) {
                Ok(Some(text)) if !text.trim().is_empty() => {
                    events.push(TranscriptEvent::Final {
                        speaker: self.speaker,
                        text,
                    })
                }
                Ok(_) => {}
                Err(err) => self.push_error(&mut events, err.to_string()),
            }
        }

        match self.engine.finish() {
            Ok(Some(text)) if !text.trim().is_empty() => events.push(TranscriptEvent::Final {
                speaker: self.speaker,
                text,
            }),
            Ok(_) => {}
            Err(err) => self.push_error(&mut events, err.to_string()),
        }

        events
    }

    /// 相同错误只上报一次，避免部分结果刷新频率导致刷屏
    fn push_error(&mut self, events: &mut Vec<TranscriptEvent>, message: String) {
        if self.last_error.as_deref() == Some(message.as_str()) {
            return;
        }
        self.last_error = Some(message.clone());
        events.push(TranscriptEvent::Error { message });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asr::engine::ReplayEngine;
    use crate::providers::types::ProviderError;

    const TS_STEP: i64 = FRAME_MS as i64;

    fn speech_frame() -> Vec<f32> {
        (0..FRAME_SAMPLES)
            .map(|i| if i % 2 == 0 { 0.8 } else { -0.8 })
            .collect()
    }

    fn silence_frame() -> Vec<f32> {
        vec![0.0; FRAME_SAMPLES]
    }

    /// 计数型引擎：便于断言调用次数与 is_final 语义
    struct CountingEngine {
        partial_calls: usize,
        final_calls: usize,
    }

    impl LocalAsrEngine for CountingEngine {
        fn name(&self) -> &str {
            "counting"
        }

        fn transcribe_segment(
            &mut self,
            samples: &[f32],
            is_final: bool,
        ) -> crate::providers::types::ProviderResult<Option<String>> {
            if is_final {
                self.final_calls += 1;
                Ok(Some(format!("final-{}", self.final_calls)))
            } else {
                self.partial_calls += 1;
                Ok(Some(format!("partial-{}-{}", self.partial_calls, samples.len())))
            }
        }

        fn finish(&mut self) -> crate::providers::types::ProviderResult<Option<String>> {
            Ok(None)
        }
    }

    /// 始终失败的引擎
    struct FailingEngine;

    impl LocalAsrEngine for FailingEngine {
        fn name(&self) -> &str {
            "failing"
        }

        fn transcribe_segment(
            &mut self,
            _samples: &[f32],
            _is_final: bool,
        ) -> crate::providers::types::ProviderResult<Option<String>> {
            Err(ProviderError::Other("模型未下载".into()))
        }

        fn finish(&mut self) -> crate::providers::types::ProviderResult<Option<String>> {
            Ok(None)
        }
    }

    fn collect_finals(events: &[TranscriptEvent]) -> Vec<String> {
        events
            .iter()
            .filter_map(|event| match event {
                TranscriptEvent::Final { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn pushes_final_for_completed_segment() {
        let engine = ReplayEngine::with_script(vec!["面试官问了项目架构".into()]);
        let mut pipeline =
            AsrPipeline::with_defaults(Box::new(engine), Speaker::Interviewer, 0.5);

        let mut events = Vec::new();
        // 30 帧语音（600ms）+ 30 帧静音（600ms > min_silence 400ms）→ 收段
        for _ in 0..30 {
            events.extend(pipeline.push_frame(&speech_frame(), 0));
        }
        for _ in 0..30 {
            events.extend(pipeline.push_frame(&silence_frame(), 0));
        }

        let finals = collect_finals(&events);
        assert_eq!(finals, vec!["面试官问了项目架构".to_string()]);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, TranscriptEvent::Partial { .. })),
            "应产出至少一个 partial"
        );
    }

    #[test]
    fn partial_is_emitted_at_configured_interval() {
        let engine = CountingEngine {
            partial_calls: 0,
            final_calls: 0,
        };
        // 间隔 200ms = 10 帧；先喂 10 帧语音建立确认段（min_speech=200ms）
        let mut pipeline = AsrPipeline::new(
            Box::new(engine),
            Speaker::Me,
            VadConfig::default(),
            0.5,
            200,
        );

        let mut partials = 0usize;
        for _ in 0..30 {
            for event in pipeline.push_frame(&speech_frame(), 0) {
                if matches!(event, TranscriptEvent::Partial { .. }) {
                    partials += 1;
                }
            }
        }
        assert!(partials >= 2, "600ms 语音按 200ms 间隔应产出多个 partial：{partials}");
    }

    #[test]
    fn identical_partial_is_not_repeated() {
        // 回放引擎在同一段内返回相同文本，应只上报一次 partial
        let engine = ReplayEngine::with_script(vec!["重复文本".into()]);
        let mut pipeline = AsrPipeline::new(
            Box::new(engine),
            Speaker::Me,
            VadConfig::default(),
            0.5,
            100,
        );

        let mut partials = 0usize;
        for _ in 0..30 {
            for event in pipeline.push_frame(&speech_frame(), 0) {
                if matches!(event, TranscriptEvent::Partial { .. }) {
                    partials += 1;
                }
            }
        }
        assert_eq!(partials, 1, "相同 partial 不应重复上报");
    }

    #[test]
    fn flush_emits_pending_speech_as_final() {
        let engine = ReplayEngine::with_script(vec!["未结束的语音".into()]);
        let mut pipeline =
            AsrPipeline::with_defaults(Box::new(engine), Speaker::Interviewer, 0.5);

        for _ in 0..20 {
            pipeline.push_frame(&speech_frame(), 0);
        }
        let events = pipeline.flush();
        assert_eq!(collect_finals(&events), vec!["未结束的语音".to_string()]);

        // 二次 flush 不应重复产出
        assert!(collect_finals(&pipeline.flush()).is_empty());
    }

    #[test]
    fn silence_only_stream_produces_no_final() {
        let engine = ReplayEngine::with_script(vec!["不应出现".into()]);
        let mut pipeline =
            AsrPipeline::with_defaults(Box::new(engine), Speaker::Me, 0.5);

        let mut events = Vec::new();
        for _ in 0..50 {
            events.extend(pipeline.push_frame(&silence_frame(), 0));
        }
        events.extend(pipeline.flush());
        assert!(collect_finals(&events).is_empty());
    }

    #[test]
    fn engine_error_is_surfaced_once_and_deduped() {
        let mut pipeline = AsrPipeline::new(
            Box::new(FailingEngine),
            Speaker::Me,
            VadConfig::default(),
            0.5,
            100,
        );

        let mut errors = Vec::new();
        for _ in 0..40 {
            for event in pipeline.push_frame(&speech_frame(), 0) {
                if let TranscriptEvent::Error { message } = event {
                    errors.push(message);
                }
            }
        }

        assert_eq!(errors.len(), 1, "相同错误应去重：{errors:?}");
        assert!(errors[0].contains("模型未下载"));
    }

    #[test]
    fn short_frame_is_ignored() {
        let engine = CountingEngine {
            partial_calls: 0,
            final_calls: 0,
        };
        let mut pipeline =
            AsrPipeline::with_defaults(Box::new(engine), Speaker::Me, 0.5);
        assert!(pipeline.push_frame(&vec![0.5; 10], 0).is_empty());
    }

    #[test]
    fn speaker_and_engine_name_are_exposed() {
        let engine = ReplayEngine::new();
        let pipeline = AsrPipeline::with_defaults(Box::new(engine), Speaker::Interviewer, 0.5);
        assert_eq!(pipeline.speaker(), Speaker::Interviewer);
        assert_eq!(pipeline.engine_name(), "replay");
    }

    #[test]
    fn multiple_utterances_produce_ordered_finals() {
        let engine = ReplayEngine::with_script(vec!["第一个问题".into(), "第二个问题".into()]);
        let mut pipeline =
            AsrPipeline::with_defaults(Box::new(engine), Speaker::Interviewer, 0.5);

        let mut events = Vec::new();
        for _ in 0..2 {
            for _ in 0..30 {
                events.extend(pipeline.push_frame(&speech_frame(), TS_STEP));
            }
            for _ in 0..30 {
                events.extend(pipeline.push_frame(&silence_frame(), TS_STEP));
            }
        }

        assert_eq!(
            collect_finals(&events),
            vec!["第一个问题".to_string(), "第二个问题".to_string()]
        );
    }
}
