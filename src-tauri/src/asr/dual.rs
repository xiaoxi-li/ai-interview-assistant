//! 双通道会话（IA-05 说话人区分 + IA-02 回声消除接入）。
//!
//! 同时采集两路信号，天然区分说话人、无需额外的声纹分离模型：
//! * **麦克风通道** → 说话人「我」，先经回声消除（以系统音频为参考）再转写；
//! * **系统音频通道** → 说话人「面试官」，直接转写。
//!
//! 本模块不依赖 Tauri/数据库，可完全用 PCM 回放做单元测试；
//! 命令层只负责把采集事件喂进来、把事件广播出去/落库。

use super::engine::LocalAsrEngine;
use super::pipeline::AsrPipeline;
use crate::audio::aec::{AecConfig, EchoCanceller};
use crate::audio::TARGET_SAMPLE_RATE;
use crate::providers::types::{Speaker, TranscriptEvent};
use std::collections::VecDeque;

/// 参考信号缓冲窗口（毫秒）：覆盖两路采集的时间对齐偏差
pub const REFERENCE_WINDOW_MS: i64 = 500;

/// 按时间戳缓存参考帧（系统音频），供 AEC 取用
struct ReferenceBuffer {
    frames: VecDeque<(i64, Vec<f32>)>,
    window_ms: i64,
}

impl ReferenceBuffer {
    fn new(window_ms: i64) -> Self {
        Self {
            frames: VecDeque::new(),
            window_ms: window_ms.max(1),
        }
    }

    fn push(&mut self, ts: i64, samples: Vec<f32>) {
        self.frames.push_back((ts, samples));
        self.prune(ts);
    }

    fn len(&self) -> usize {
        self.frames.len()
    }

    /// 取出与 `ts` 对应时刻最合适的参考帧：优先「不晚于 ts 的最后一帧」，
    /// 若系统通道暂时落后则退化为最早一帧（避免无参考可用）。
    fn take_nearest(&mut self, ts: i64) -> Option<Vec<f32>> {
        if self.frames.is_empty() {
            return None;
        }

        let mut candidate: Option<usize> = None;
        for (index, (frame_ts, _)) in self.frames.iter().enumerate() {
            if *frame_ts <= ts {
                candidate = Some(index);
            } else {
                break;
            }
        }
        let index = candidate.unwrap_or(0);
        self.frames.remove(index).map(|(_, samples)| samples)
    }

    /// 丢弃过旧帧，避免缓冲无限增长
    fn prune(&mut self, now_ts: i64) {
        let cutoff = now_ts - self.window_ms;
        while let Some((ts, _)) = self.frames.front() {
            if *ts < cutoff {
                self.frames.pop_front();
            } else {
                break;
            }
        }
    }
}

/// 双通道会话：两条独立转写管线 + 麦克风通道回声消除
pub struct DualChannelSession {
    mic: AsrPipeline,
    system: AsrPipeline,
    aec: Option<EchoCanceller>,
    reference: ReferenceBuffer,
    mic_frames: u64,
    system_frames: u64,
}

impl DualChannelSession {
    pub fn new(
        mic_engine: Box<dyn LocalAsrEngine>,
        system_engine: Box<dyn LocalAsrEngine>,
        vad_threshold: f32,
        aec_config: Option<AecConfig>,
    ) -> Self {
        Self {
            mic: AsrPipeline::with_defaults(mic_engine, Speaker::Me, vad_threshold),
            system: AsrPipeline::with_defaults(system_engine, Speaker::Interviewer, vad_threshold),
            aec: aec_config.map(|config| EchoCanceller::new(config, TARGET_SAMPLE_RATE)),
            reference: ReferenceBuffer::new(REFERENCE_WINDOW_MS),
            mic_frames: 0,
            system_frames: 0,
        }
    }

    pub fn aec_enabled(&self) -> bool {
        self.aec.is_some()
    }

    /// 回声抑制量（dB）；未启用 AEC 时为 None
    pub fn aec_erle_db(&self) -> Option<f32> {
        self.aec.as_ref().map(EchoCanceller::erle_db)
    }

    pub fn mic_frames(&self) -> u64 {
        self.mic_frames
    }

    pub fn system_frames(&self) -> u64 {
        self.system_frames
    }

    /// 参考缓冲待处理帧数（诊断用）
    pub fn pending_reference_frames(&self) -> usize {
        self.reference.len()
    }

    /// 送入一帧**系统音频**：既作为 AEC 参考，也进入「面试官」通道转写
    pub fn push_system_frame(&mut self, samples: &[f32], ts: i64) -> Vec<TranscriptEvent> {
        if samples.is_empty() {
            return Vec::new();
        }
        self.system_frames += 1;
        self.reference.push(ts, samples.to_vec());
        self.system.push_frame(samples, ts)
    }

    /// 送入一帧**麦克风音频**：先做回声消除，再进入「我」的通道转写
    pub fn push_mic_frame(&mut self, samples: &[f32], ts: i64) -> Vec<TranscriptEvent> {
        if samples.is_empty() {
            return Vec::new();
        }
        self.mic_frames += 1;

        let processed = match self.aec.as_mut() {
            Some(aec) => match self.reference.take_nearest(ts) {
                // 有参考信号：消除回声
                Some(reference) => aec.process(samples, &reference),
                // 无参考（系统通道未启动/落后）：直通，避免有损处理
                None => samples.to_vec(),
            },
            None => samples.to_vec(),
        };

        self.mic.push_frame(&processed, ts)
    }

    /// 音频结束：冲刷两条管线的未完成语音段
    pub fn flush(&mut self) -> Vec<TranscriptEvent> {
        let mut events = self.system.flush();
        events.extend(self.mic.flush());
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::aec::{DEFAULT_DTD_RATIO, DEFAULT_STEP_SIZE};
    use crate::audio::FRAME_SAMPLES;
    use crate::asr::engine::ReplayEngine;

    const FRAME: usize = FRAME_SAMPLES;
    const TS_STEP: i64 = 20;

    fn aec_config() -> AecConfig {
        AecConfig {
            enabled: true,
            filter_len_ms: 64,
            step_size: DEFAULT_STEP_SIZE,
            dtd_ratio: DEFAULT_DTD_RATIO,
        }
    }

    fn speech_frame() -> Vec<f32> {
        (0..FRAME)
            .map(|index| if index % 2 == 0 { 0.8 } else { -0.8 })
            .collect()
    }

    fn silence_frame() -> Vec<f32> {
        vec![0.0; FRAME]
    }

    struct Lcg(u64);
    impl Lcg {
        fn new(seed: u64) -> Self {
            Self(seed)
        }
        fn next_f32(&mut self) -> f32 {
            self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            ((self.0 >> 40) as f32 / 8_388_608.0) - 1.0
        }
    }

    /// 纯延迟回声（恰好可被 NLMS 完全建模）
    fn delayed_echo(reference: &[f32], delay: usize, gain: f32) -> Vec<f32> {
        let mut out = vec![0.0f32; reference.len()];
        for index in delay..reference.len() {
            out[index] = gain * reference[index - delay];
        }
        out
    }

    fn finals_of(events: &[TranscriptEvent]) -> Vec<(Speaker, String)> {
        events
            .iter()
            .filter_map(|event| match event {
                TranscriptEvent::Final { speaker, text } => Some((*speaker, text.clone())),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn reference_buffer_prefers_latest_not_later_than_mic() {
        let mut buffer = ReferenceBuffer::new(REFERENCE_WINDOW_MS);
        buffer.push(0, vec![0.0]);
        buffer.push(20, vec![1.0]);
        buffer.push(40, vec![2.0]);
        buffer.push(60, vec![3.0]);

        // mic 在 50ms：应取 40ms 那帧（已播放且最近）
        assert_eq!(buffer.take_nearest(50).expect("frame"), vec![2.0]);
        // mic 落后于系统时退化为最早一帧
        assert_eq!(buffer.take_nearest(-10).expect("frame"), vec![0.0]);
    }

    #[test]
    fn reference_buffer_prunes_old_frames() {
        let mut buffer = ReferenceBuffer::new(100);
        buffer.push(0, vec![0.0]);
        buffer.push(200, vec![1.0]);
        assert_eq!(buffer.len(), 1, "超出窗口的旧帧应被丢弃");
    }

    #[test]
    fn reference_buffer_is_empty_initially() {
        let mut buffer = ReferenceBuffer::new(REFERENCE_WINDOW_MS);
        assert_eq!(buffer.len(), 0);
        assert!(buffer.take_nearest(0).is_none());
    }

    #[test]
    fn dual_streams_are_labelled_with_correct_speakers() {
        let mut session = DualChannelSession::new(
            Box::new(ReplayEngine::with_script(vec!["我的回答".into()])),
            Box::new(ReplayEngine::with_script(vec!["面试官的问题".into()])),
            0.5,
            None,
        );

        // 注意：时间戳必须单调递增（VAD 依赖 ts 划分语音段）
        let mut events = Vec::new();
        let mut clock = 0i64;
        for _round in 0..2 {
            for _ in 0..30 {
                clock += TS_STEP;
                events.extend(session.push_system_frame(&speech_frame(), clock));
                events.extend(session.push_mic_frame(&speech_frame(), clock));
            }
            for _ in 0..30 {
                clock += TS_STEP;
                // 注意：语音段是在静音期收盘的，返回的 final 必须收集
                events.extend(session.push_system_frame(&silence_frame(), clock));
                events.extend(session.push_mic_frame(&silence_frame(), clock));
            }
        }
        events.extend(session.flush());

        let finals = finals_of(&events);
        let interviewer: Vec<&String> = finals
            .iter()
            .filter(|(speaker, _)| *speaker == Speaker::Interviewer)
            .map(|(_, text)| text)
            .collect();
        let me: Vec<&String> = finals
            .iter()
            .filter(|(speaker, _)| *speaker == Speaker::Me)
            .map(|(_, text)| text)
            .collect();

        assert!(
            interviewer.contains(&&"面试官的问题".to_string()),
            "面试官通道应转写出脚本内容，实际 {finals:?}"
        );
        assert!(
            me.contains(&&"我的回答".to_string()),
            "麦克风通道应转写出脚本内容，实际 {finals:?}"
        );
        assert_eq!(session.mic_frames(), 120);
        assert_eq!(session.system_frames(), 120);
    }

    #[test]
    fn aec_suppresses_echo_on_mic_channel() {
        let mut session = DualChannelSession::new(
            Box::new(ReplayEngine::new()),
            Box::new(ReplayEngine::new()),
            0.5,
            Some(aec_config()),
        );
        assert!(session.aec_enabled());

        let frames = 120;
        let mut rng = Lcg::new(9);
        let reference: Vec<f32> = (0..frames * FRAME).map(|_| 0.4 * rng.next_f32()).collect();
        let echo = delayed_echo(&reference, FRAME, 0.5); // 20ms 延迟回声

        for frame in 0..frames {
            let start = frame * FRAME;
            let ts = (frame as i64) * TS_STEP;
            session.push_system_frame(&reference[start..start + FRAME], ts);

            let mic: Vec<f32> = echo[start..start + FRAME]
                .iter()
                .map(|sample| sample + 0.0005 * rng.next_f32())
                .collect();
            session.push_mic_frame(&mic, ts);
        }

        let erle = session.aec_erle_db().expect("erle");
        assert!(erle > 15.0, "麦克风通道应获得明显回声抑制，实际 {erle:.1}dB");
        assert_eq!(session.pending_reference_frames(), 0, "参考帧应被消费");
    }

    #[test]
    fn mic_channel_passes_through_without_reference() {
        let mut session = DualChannelSession::new(
            Box::new(ReplayEngine::new()),
            Box::new(ReplayEngine::new()),
            0.5,
            Some(aec_config()),
        );

        // 只有麦克风，没有系统音频 → 无参考，应直通且不产生错误
        let mut events = Vec::new();
        for index in 0..40 {
            events.extend(session.push_mic_frame(&speech_frame(), (index as i64) * TS_STEP));
        }
        assert!(session.pending_reference_frames() == 0);
        // 语音段应正常进入「我」的通道
        assert!(events.iter().any(|event| matches!(
            event,
            TranscriptEvent::Final {
                speaker: Speaker::Me,
                ..
            } | TranscriptEvent::Partial {
                speaker: Speaker::Me,
                ..
            }
        )));
    }

    #[test]
    fn aec_disabled_reports_none_and_keeps_channels_working() {
        let mut session = DualChannelSession::new(
            Box::new(ReplayEngine::new()),
            Box::new(ReplayEngine::new()),
            0.5,
            None,
        );
        assert!(!session.aec_enabled());
        assert!(session.aec_erle_db().is_none());

        let mut events = Vec::new();
        for index in 0..40 {
            let ts = (index as i64) * TS_STEP;
            session.push_system_frame(&speech_frame(), ts);
            events.extend(session.push_mic_frame(&speech_frame(), ts));
        }
        assert!(events
            .iter()
            .any(|event| matches!(event, TranscriptEvent::Partial { .. })));
    }

    #[test]
    fn flush_collects_pending_segments_from_both_channels() {
        let mut session = DualChannelSession::new(
            Box::new(ReplayEngine::with_script(vec!["我的未完成语音".into()])),
            Box::new(ReplayEngine::with_script(vec!["面试官未完成语音".into()])),
            0.5,
            None,
        );

        for index in 0..20 {
            let ts = (index as i64) * TS_STEP;
            session.push_system_frame(&speech_frame(), ts);
            session.push_mic_frame(&speech_frame(), ts);
        }

        let finals = finals_of(&session.flush());
        assert!(finals.contains(&(Speaker::Me, "我的未完成语音".to_string())));
        assert!(finals.contains(&(Speaker::Interviewer, "面试官未完成语音".to_string())));
        // 二次 flush 不应重复产出
        assert!(finals_of(&session.flush()).is_empty());
    }

    #[test]
    fn empty_frames_are_ignored() {
        let mut session = DualChannelSession::new(
            Box::new(ReplayEngine::new()),
            Box::new(ReplayEngine::new()),
            0.5,
            None,
        );
        assert!(session.push_mic_frame(&[], 0).is_empty());
        assert!(session.push_system_frame(&[], 0).is_empty());
        assert_eq!(session.mic_frames(), 0);
        assert_eq!(session.system_frames(), 0);
    }
}
