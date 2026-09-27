//! VAD（语音活动检测）与自动分段。
//!
//! 设计：
//! * [`VoiceActivityDetector`] 抽象单帧判定，P0 提供基于能量的 [`EnergyVad`]，
//!   后续可无缝替换为 silero-vad（ONNX）实现；
//! * [`VadSegmenter`] 负责状态机与分段：连续语音超过 `min_speech_ms` 才起段，
//!   静音超过 `min_silence_ms` 才收段，并附带前后 padding，避免首尾被截断。

use super::capture::{rms_level, FRAME_MS, FRAME_SAMPLES};

/// 单帧判定接口
pub trait VoiceActivityDetector: Send {
    /// 判断该帧是否包含语音
    fn is_speech(&self, frame: &[f32]) -> bool;
}

/// 界面的 VAD 阈值（0~1，越大越严格）→ 绝对 RMS 阈值。
///
/// **为什么需要映射**：真实语音与系统回环音频的 RMS 通常只有 0.01~0.2
/// （约 -40 ~ -14 dBFS），而界面滑杆是按 0~1 取值的。若把滑杆值直接当 RMS 用，
/// 只有极端音量（RMS ≥ 0.5 ≈ -6 dBFS）才会被判为语音，等于永远检测不到说话
/// ——这正是「电平条在跳、却没有任何转写、也没有报错」的根因。
///
/// 因此按对数（dBFS）映射，让滑杆全程都落在有效区间：
/// `0 → -60 dBFS (0.001 最灵敏)`、`0.5 → -33 dBFS (0.022，适合正常说话音量)`、
/// `1 → -6 dBFS (0.5 最严格)`。
pub fn threshold_to_rms(threshold: f32) -> f32 {
    const MIN_DB: f32 = -60.0;
    const MAX_DB: f32 = -6.0;
    let t = threshold.clamp(0.0, 1.0);
    10f32.powf((MIN_DB + t * (MAX_DB - MIN_DB)) / 20.0)
}

/// 基于短时能量的 VAD
#[derive(Debug, Clone)]
pub struct EnergyVad {
    /// 界面语义的灵敏度（0~1，越大越严格）
    threshold: f32,
}

impl EnergyVad {
    pub fn new(threshold: f32) -> Self {
        Self {
            threshold: threshold.clamp(0.0, 1.0),
        }
    }

    /// 界面的灵敏度值（0~1）
    pub fn threshold(&self) -> f32 {
        self.threshold
    }

    /// 该灵敏度对应的绝对 RMS 判定门限
    pub fn rms_threshold(&self) -> f32 {
        threshold_to_rms(self.threshold)
    }
}

impl Default for EnergyVad {
    fn default() -> Self {
        Self::new(0.5)
    }
}

impl VoiceActivityDetector for EnergyVad {
    fn is_speech(&self, frame: &[f32]) -> bool {
        rms_level(frame) >= self.rms_threshold()
    }
}

/// 分段参数
#[derive(Debug, Clone, PartialEq)]
pub struct VadConfig {
    /// 连续语音达到该时长才确认为一段（毫秒）
    pub min_speech_ms: u32,
    /// 连续静音达到该时长才结束当前段（毫秒）
    pub min_silence_ms: u32,
    /// 段首尾额外保留的时长（毫秒），避免吞掉起止音
    pub padding_ms: u32,
    /// 单段最大时长（毫秒），超长会自动切段，避免无限增长
    pub max_segment_ms: u32,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            min_speech_ms: 200,
            min_silence_ms: 400,
            padding_ms: 100,
            max_segment_ms: 15_000,
        }
    }
}

impl VadConfig {
    /// 毫秒换算为帧数（向上取整）；0 毫秒表示不保留，必须返回 0 而非 1
    fn frames(&self, ms: u32) -> u32 {
        if ms == 0 {
            return 0;
        }
        ms.div_ceil(FRAME_MS)
    }
}

/// 已识别的语音段
#[derive(Debug, Clone, PartialEq)]
pub struct VadSegment {
    pub samples: Vec<f32>,
    pub started_at: i64,
    pub ended_at: i64,
}

impl VadSegment {
    pub fn duration_ms(&self) -> u32 {
        ((self.samples.len() as u32) * 1000) / super::capture::TARGET_SAMPLE_RATE
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SegmenterState {
    Idle,
    MaybeSpeech,
    Speech,
    MaybeSilence,
}

/// VAD 分段状态机
pub struct VadSegmenter {
    detector: Box<dyn VoiceActivityDetector>,
    config: VadConfig,
    state: SegmenterState,
    /// 当前段样本（Speech 状态下累积）
    current: Vec<f32>,
    /// 预滚缓冲：保留 Idle 状态下的最近若干帧，用于补齐段首
    preroll: Vec<f32>,
    speech_frames: u32,
    silence_frames: u32,
    /// 段首 padding 需要的帧数
    padding_frames: u32,
    /// 段尾 padding 需要的帧数
    tail_frames: u32,
    segment_start_ts: i64,
    last_ts: i64,
}

impl VadSegmenter {
    pub fn new(config: VadConfig, detector: Box<dyn VoiceActivityDetector>) -> Self {
        let padding_frames = config.frames(config.padding_ms);
        Self {
            detector,
            tail_frames: padding_frames,
            padding_frames,
            config,
            state: SegmenterState::Idle,
            current: Vec::new(),
            preroll: Vec::new(),
            speech_frames: 0,
            silence_frames: 0,
            segment_start_ts: 0,
            last_ts: 0,
        }
    }

    /// 使用默认能量 VAD 构造
    pub fn with_energy_vad(config: VadConfig, threshold: f32) -> Self {
        Self::new(config, Box::new(EnergyVad::new(threshold)))
    }

    fn reset(&mut self) {
        self.state = SegmenterState::Idle;
        self.current.clear();
        self.preroll.clear();
        self.speech_frames = 0;
        self.silence_frames = 0;
    }

    /// 将预滚缓冲写入当前段（补齐段首 padding）
    fn start_segment(&mut self, ts: i64) {
        let preroll = std::mem::take(&mut self.preroll);
        self.current = preroll;
        self.segment_start_ts = ts;
        self.state = SegmenterState::Speech;
        self.speech_frames = 0;
        self.silence_frames = 0;
    }

    fn finish_segment(&mut self, ts: i64) -> Option<VadSegment> {
        if self.current.is_empty() {
            self.reset();
            return None;
        }
        let samples = std::mem::take(&mut self.current);
        let segment = VadSegment {
            samples,
            started_at: self.segment_start_ts,
            ended_at: ts,
        };
        self.reset();
        Some(segment)
    }

    /// 送入一帧（16kHz 单声道，320 样本），返回本次产生的完整语音段
    pub fn push_frame(&mut self, frame: &[f32], ts: i64) -> Option<VadSegment> {
        self.last_ts = ts;
        let is_speech = self.detector.is_speech(frame);
        let min_speech = self.config.frames(self.config.min_speech_ms);
        let min_silence = self.config.frames(self.config.min_silence_ms);
        let max_frames = self.config.frames(self.config.max_segment_ms);

        match self.state {
            SegmenterState::Idle => {
                // 维持预滚窗口（padding 帧数）
                self.preroll.extend_from_slice(frame);
                let cap = self.padding_frames as usize * FRAME_SAMPLES;
                if self.preroll.len() > cap {
                    let overflow = self.preroll.len() - cap;
                    self.preroll.drain(..overflow);
                }

                if is_speech {
                    self.state = SegmenterState::MaybeSpeech;
                    self.speech_frames = 1;
                    self.current = std::mem::take(&mut self.preroll);
                    self.current.extend_from_slice(frame);
                    self.segment_start_ts = ts;
                }
                None
            }

            SegmenterState::MaybeSpeech => {
                self.current.extend_from_slice(frame);
                if is_speech {
                    self.speech_frames += 1;
                    if self.speech_frames >= min_speech {
                        self.state = SegmenterState::Speech;
                        self.silence_frames = 0;
                    }
                } else {
                    // 语音过短，视为噪声，丢弃并回到 Idle
                    self.current.clear();
                    self.preroll.clear();
                    self.state = SegmenterState::Idle;
                    self.speech_frames = 0;
                }
                None
            }

            SegmenterState::Speech => {
                self.current.extend_from_slice(frame);
                let frames_in_segment = (self.current.len() / FRAME_SAMPLES) as u32;
                if frames_in_segment >= max_frames {
                    // 超长自动切段
                    return self.finish_segment(ts);
                }
                if is_speech {
                    self.silence_frames = 0;
                } else {
                    self.silence_frames = 1;
                    self.state = SegmenterState::MaybeSilence;
                }
                None
            }

            SegmenterState::MaybeSilence => {
                self.current.extend_from_slice(frame);
                if is_speech {
                    // 只是短暂停顿，继续累积
                    self.state = SegmenterState::Speech;
                    self.silence_frames = 0;
                    return None;
                }

                self.silence_frames += 1;
                if self.silence_frames >= min_silence {
                    return self.finish_segment(ts);
                }
                None
            }
        }
    }

    /// 流结束时调用：把仍在进行中的语音段吐出
    pub fn flush(&mut self) -> Option<VadSegment> {
        if matches!(
            self.state,
            SegmenterState::Speech | SegmenterState::MaybeSilence
        ) {
            return self.finish_segment(self.last_ts);
        }
        self.reset();
        None
    }

    /// 段尾 padding 帧数（供上层在静音期保留尾巴）
    pub fn tail_padding_frames(&self) -> u32 {
        self.tail_frames
    }

    /// 当前进行中的语音段样本（供上层产出 partial 转写结果）
    pub fn current_segment_samples(&self) -> &[f32] {
        &self.current
    }

    /// 是否已确认处于语音段中（MaybeSpeech 不算，避免噪声触发 partial）
    pub fn is_in_confirmed_speech(&self) -> bool {
        matches!(
            self.state,
            SegmenterState::Speech | SegmenterState::MaybeSilence
        )
    }
}

/// 单段摘要（供诊断报告使用）
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VadSegmentSummary {
    pub duration_ms: u32,
    pub started_at: i64,
    pub ended_at: i64,
}

/// VAD 诊断报告
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VadReport {
    /// 输入总时长（毫秒）
    pub total_ms: u32,
    /// 识别为语音的时长（毫秒）
    pub speech_ms: u32,
    /// 语音占比（0~1）
    pub speech_ratio: f32,
    pub threshold: f32,
    pub segments: Vec<VadSegmentSummary>,
}

/// 对整段 16kHz 单声道音频执行 VAD 分段（诊断/自检用）
pub fn analyze(samples: &[f32], threshold: f32) -> VadReport {
    let mut segmenter = VadSegmenter::with_energy_vad(VadConfig::default(), threshold);
    let mut segments: Vec<VadSegment> = Vec::new();
    let mut ts = 0i64;

    for frame in samples.chunks(FRAME_SAMPLES) {
        if frame.len() < FRAME_SAMPLES {
            break;
        }
        ts += FRAME_MS as i64;
        if let Some(segment) = segmenter.push_frame(frame, ts) {
            segments.push(segment);
        }
    }
    if let Some(last) = segmenter.flush() {
        segments.push(last);
    }

    // 仅统计可完整成帧的部分，避免尾部残缺帧影响占比
    let usable_samples = (samples.len() / FRAME_SAMPLES) * FRAME_SAMPLES;
    let total_ms = ((usable_samples as u32) * 1000) / super::capture::TARGET_SAMPLE_RATE;
    let speech_ms: u32 = segments.iter().map(VadSegment::duration_ms).sum();

    VadReport {
        total_ms,
        speech_ms,
        speech_ratio: if total_ms == 0 {
            0.0
        } else {
            speech_ms as f32 / total_ms as f32
        },
        threshold,
        segments: segments
            .into_iter()
            .map(|segment| VadSegmentSummary {
                duration_ms: segment.duration_ms(),
                started_at: segment.started_at,
                ended_at: segment.ended_at,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TS_STEP: i64 = FRAME_MS as i64;

    fn speech_frame() -> Vec<f32> {
        // 0.8 振幅方波，RMS 明显高于默认阈值
        (0..FRAME_SAMPLES)
            .map(|i| if i % 2 == 0 { 0.8 } else { -0.8 })
            .collect()
    }

    fn silence_frame() -> Vec<f32> {
        vec![0.0; FRAME_SAMPLES]
    }

    fn feed(segmenter: &mut VadSegmenter, frames: &[bool]) -> Vec<VadSegment> {
        let mut segments = Vec::new();
        let mut ts = 0i64;
        for is_speech in frames {
            let frame = if *is_speech {
                speech_frame()
            } else {
                silence_frame()
            };
            ts += TS_STEP;
            if let Some(segment) = segmenter.push_frame(&frame, ts) {
                segments.push(segment);
            }
        }
        segments
    }

    #[test]
    fn energy_vad_separates_speech_and_silence() {
        let vad = EnergyVad::new(0.5);
        assert!(vad.is_speech(&speech_frame()));
        assert!(!vad.is_speech(&silence_frame()));
    }

    #[test]
    fn energy_vad_threshold_is_clamped() {
        assert_eq!(EnergyVad::new(2.0).threshold(), 1.0);
        assert_eq!(EnergyVad::new(-1.0).threshold(), 0.0);
    }

    #[test]
    fn vad_threshold_maps_to_log_scale_rms() {
        // 0 → -60dBFS、0.5 → -33dBFS、1 → -6dBFS
        assert!((threshold_to_rms(0.0) - 0.001).abs() < 1e-4, "0 应最灵敏");
        assert!((threshold_to_rms(0.5) - 0.0224).abs() < 1e-3, "0.5 应约为 -33dBFS");
        assert!((threshold_to_rms(1.0) - 0.5012).abs() < 1e-3, "1 应最严格");
        // 单调递增（越大越严格）
        assert!(threshold_to_rms(0.2) < threshold_to_rms(0.4));
        assert!(threshold_to_rms(0.8) < threshold_to_rms(1.0));
    }

    /// 回归测试：真实录音音量（RMS 约 0.02~0.1）必须被判为语音。
    ///
    /// 历史 bug：界面 0~1 的滑杆值被直接当作绝对 RMS 门限，默认 0.5 等于要求
    /// RMS ≥ 0.5（≈ -6dBFS），导致真实语音全被当成静音 —— 现象就是「电平条在跳、
    /// 却没有任何转写、也没有报错」。
    #[test]
    fn realistic_recording_level_is_detected_as_speech() {
        let vad = EnergyVad::default(); // 界面默认 0.5 → ≈ -33dBFS

        // 正常说话音量：正弦振幅 0.07 → RMS ≈ 0.05
        let normal: Vec<f32> = (0..FRAME_SAMPLES)
            .map(|i| 0.07 * (i as f32 * 0.2).sin())
            .collect();
        assert!(vad.is_speech(&normal), "正常音量应判为语音");

        // 偏低音量（如系统回环音量不大）：振幅 0.04 → RMS ≈ 0.028
        let quiet: Vec<f32> = (0..FRAME_SAMPLES)
            .map(|i| 0.04 * (i as f32 * 0.2).sin())
            .collect();
        assert!(vad.is_speech(&quiet), "偏低音量也应判为语音");

        // 底噪：振幅 0.007 → RMS ≈ 0.005，应被过滤
        let noise: Vec<f32> = (0..FRAME_SAMPLES)
            .map(|i| 0.007 * (i as f32 * 0.2).sin())
            .collect();
        assert!(!vad.is_speech(&noise), "底噪不应判为语音");
    }

    #[test]
    fn pure_silence_produces_no_segment() {
        let mut segmenter = VadSegmenter::with_energy_vad(VadConfig::default(), 0.5);
        // 1 秒静音
        let segments = feed(&mut segmenter, &[false; 50]);
        assert!(segments.is_empty());
        assert!(segmenter.flush().is_none());
    }

    #[test]
    fn short_blip_is_rejected_as_noise() {
        let mut segmenter = VadSegmenter::with_energy_vad(VadConfig::default(), 0.5);
        // 仅 1 帧语音（20ms）< min_speech_ms(200ms)
        let segments = feed(&mut segmenter, &[true, false, false, false, false]);
        assert!(segments.is_empty(), "过短语音应被过滤");
    }

    #[test]
    fn continuous_speech_is_emitted_on_flush() {
        let mut segmenter = VadSegmenter::with_energy_vad(VadConfig::default(), 0.5);
        let segments = feed(&mut segmenter, &[true; 40]);
        assert!(segments.is_empty(), "流未结束时不应提前收段");

        let segment = segmenter.flush().expect("flush 应吐出进行中的段");
        assert!(segment.duration_ms() >= 800, "段时长应覆盖输入");
        assert!(segment.ended_at > segment.started_at);
    }

    #[test]
    fn two_utterances_produce_two_segments() {
        let mut segmenter = VadSegmenter::with_energy_vad(VadConfig::default(), 0.5);
        let mut frames = vec![true; 30]; // 第一段 600ms
        frames.extend(vec![false; 30]); // 静音 600ms > min_silence(400ms)
        frames.extend(vec![true; 30]); // 第二段
        frames.extend(vec![false; 30]);

        let mut segments = feed(&mut segmenter, &frames);
        if let Some(last) = segmenter.flush() {
            segments.push(last);
        }

        assert_eq!(segments.len(), 2, "应切出两段语音");
        assert!(segments[0].ended_at < segments[1].started_at);
        for segment in &segments {
            assert!(
                segment.duration_ms() >= 500,
                "段应包含主要语音，实际 {}ms",
                segment.duration_ms()
            );
        }
    }

    #[test]
    fn brief_pause_does_not_split_segment() {
        let mut segmenter = VadSegmenter::with_energy_vad(VadConfig::default(), 0.5);
        let mut frames = vec![true; 30];
        frames.extend(vec![false; 10]); // 200ms 静音 < min_silence(400ms)
        frames.extend(vec![true; 30]);

        let mut segments = feed(&mut segmenter, &frames);
        if let Some(last) = segmenter.flush() {
            segments.push(last);
        }
        assert_eq!(segments.len(), 1, "短暂停顿不应切分");
    }

    #[test]
    fn long_speech_is_split_at_max_segment_length() {
        // padding 设为 0 以便精确断言长度上限（padding 会让段略长于 max）
        let config = VadConfig {
            max_segment_ms: 200,
            padding_ms: 0,
            ..VadConfig::default()
        };
        let mut segmenter = VadSegmenter::with_energy_vad(config, 0.5);
        let segments = feed(&mut segmenter, &[true; 30]); // 600ms 连续语音

        assert!(segments.len() >= 2, "超长语音应自动切段");
        for segment in &segments {
            assert!(
                segment.duration_ms() <= 220,
                "单段不应超过 max_segment_ms，实际 {}ms",
                segment.duration_ms()
            );
        }
    }

    #[test]
    fn preroll_padding_is_included() {
        let config = VadConfig {
            padding_ms: 100, // 5 帧
            min_speech_ms: 200,
            min_silence_ms: 400,
            max_segment_ms: 15_000,
        };
        let mut segmenter = VadSegmenter::with_energy_vad(config, 0.5);

        // 先给 5 帧静音建立预滚，再给语音
        let mut segments = feed(&mut segmenter, &[false; 5]);
        segments.extend(feed(&mut segmenter, &[true; 20]));
        if let Some(last) = segmenter.flush() {
            segments.push(last);
        }

        let segment = segments.first().expect("应有一段语音");
        // 至少包含 5 帧预滚 + 20 帧语音
        let frames = segment.samples.len() / FRAME_SAMPLES;
        assert!(frames >= 25, "应包含预滚 padding，实际 {frames} 帧");
    }

    #[test]
    fn segment_duration_matches_sample_count() {
        let mut segmenter = VadSegmenter::with_energy_vad(VadConfig::default(), 0.5);
        feed(&mut segmenter, &[true; 20]);
        let segment = segmenter.flush().expect("segment");

        let expected = (segment.samples.len() as u32 * 1000) / 16_000;
        assert_eq!(segment.duration_ms(), expected);
    }

    #[test]
    fn flush_is_idempotent() {
        let mut segmenter = VadSegmenter::with_energy_vad(VadConfig::default(), 0.5);
        feed(&mut segmenter, &[true; 20]);
        assert!(segmenter.flush().is_some());
        assert!(segmenter.flush().is_none(), "重复 flush 不应产出");
    }

    #[test]
    fn current_segment_samples_tracks_in_progress_speech() {
        let mut segmenter = VadSegmenter::with_energy_vad(VadConfig::default(), 0.5);
        assert!(segmenter.current_segment_samples().is_empty());
        assert!(!segmenter.is_in_confirmed_speech());

        // 静音阶段不应暴露进行中样本
        feed(&mut segmenter, &[false; 3]);
        assert!(segmenter.current_segment_samples().is_empty());

        // 达到 min_speech(200ms=10 帧) 后进入确认语音
        let mut ts = 0i64;
        for _ in 0..10 {
            ts += TS_STEP;
            segmenter.push_frame(&speech_frame(), ts);
        }
        assert!(segmenter.is_in_confirmed_speech());
        assert!(
            !segmenter.current_segment_samples().is_empty(),
            "确认语音段应可用于产出 partial"
        );
    }

    #[test]
    fn short_speech_below_threshold_is_not_confirmed() {
        let mut segmenter = VadSegmenter::with_energy_vad(VadConfig::default(), 0.5);
        let mut ts = 0i64;
        for _ in 0..3 {
            ts += TS_STEP;
            segmenter.push_frame(&speech_frame(), ts);
        }
        assert!(
            !segmenter.is_in_confirmed_speech(),
            "未达 min_speech 的片段不应产出 partial"
        );
    }

    /// 拼装 16kHz 单声道测试音频：若干段语音之间插入静音
    fn build_audio(speech_frames: usize, silence_frames: usize, repeats: usize) -> Vec<f32> {
        let mut samples = Vec::new();
        for _ in 0..repeats {
            for _ in 0..speech_frames {
                samples.extend(speech_frame());
            }
            for _ in 0..silence_frames {
                samples.extend(silence_frame());
            }
        }
        samples
    }

    #[test]
    fn analyze_reports_speech_ratio_and_segments() {
        // 每轮：30 帧语音（600ms）+ 30 帧静音（600ms），共 2 轮
        let samples = build_audio(30, 30, 2);
        let report = analyze(&samples, 0.5);

        // 2 轮 × (30 帧语音 + 30 帧静音) = 120 帧 × 20ms = 2400ms
        assert_eq!(report.threshold, 0.5);
        assert_eq!(report.total_ms, 2400);
        assert_eq!(report.segments.len(), 2, "应有 2 段语音");
        assert!(report.speech_ms > 0 && report.speech_ms <= report.total_ms);
        assert!((report.speech_ratio - report.speech_ms as f32 / report.total_ms as f32).abs() < 1e-6);
    }

    #[test]
    fn analyze_silence_only_reports_zero_speech() {
        let samples = vec![0.0f32; FRAME_SAMPLES * 50];
        let report = analyze(&samples, 0.5);

        assert_eq!(report.total_ms, 1000);
        assert_eq!(report.speech_ms, 0);
        assert!(report.segments.is_empty());
        assert!(report.speech_ratio < 1e-6);
    }

    #[test]
    fn analyze_ignores_trailing_partial_frame() {
        let mut samples = vec![0.0f32; FRAME_SAMPLES * 20];
        samples.extend(vec![0.0; 100]); // 不足一帧
        let report = analyze(&samples, 0.5);
        assert_eq!(report.total_ms, 400, "不足一帧的尾部不计入分析");
    }

    #[test]
    fn analyze_empty_input_is_safe() {
        let report = analyze(&[], 0.5);
        assert_eq!(report.total_ms, 0);
        assert_eq!(report.speech_ms, 0);
        assert!(report.segments.is_empty());
    }

    #[test]
    fn higher_threshold_reduces_speech_ratio() {
        // 0.2 振幅方波（RMS = 0.2）。按对数映射：0.6 ≈ -27.6dBFS（判为语音）、
        // 0.9 ≈ -11.4dBFS（判为静音），因此低阈值段的语音时长更长。
        let medium: Vec<f32> = (0..FRAME_SAMPLES * 30)
            .map(|i| if i % 2 == 0 { 0.2 } else { -0.2 })
            .collect();

        let sensitive = analyze(&medium, 0.6);
        let strict = analyze(&medium, 0.9);

        assert!(
            sensitive.speech_ms > strict.speech_ms,
            "阈值越高判定的语音应越少：{} vs {}",
            sensitive.speech_ms,
            strict.speech_ms
        );
    }
}
