//! 回声消除（IA-02）。
//!
//! 场景：面试官声音从扬声器播放后被麦克风再次采集，形成回声。因为系统音频回环
//! （WASAPI loopback / ScreenCaptureKit）能拿到**参考信号**（远端播放内容），
//! 所以这里用经典的 **NLMS 自适应滤波**估计回声路径并从麦克风信号中减去：
//!
//! ```text
//! y[n] = wᵀ·x[n]                // 估计回声
//! e[n] = d[n] − y[n]            // 近端（我）语音估计 = 输出
//! w[n+1] = w[n] + μ·e[n]·x[n] / (ε + ‖x[n]‖²)
//! ```
//!
//! 双讲（我和面试官同时说话）会让 NLMS 发散，因此加入能量比双讲检测：
//! 当麦克风能量远高于估计回声能量（说明近端在说话）时冻结权值更新。

use serde::{Deserialize, Serialize};

/// 归一化步长：越小越稳、收敛越慢
pub const DEFAULT_STEP_SIZE: f32 = 0.25;
/// 双讲判定比值（残差功率 / 残差基线），超过则冻结自适应。
/// 取 20.0（约 26dB 突增）：双讲时残差相对单讲基线通常跃升数十 dB，
/// 该阈值既能可靠识别双讲，又不会打断正常收敛过程。
pub const DEFAULT_DTD_RATIO: f32 = 20.0;
/// 能量平滑系数（指数滑动平均）
pub const DEFAULT_SMOOTHING: f32 = 0.9;
/// 判定「远端有信号」的能量下限，避免静音时误判
pub const NOISE_FLOOR: f32 = 1e-6;
/// 残差功率基线的每样本回升速率（越小越迟钝，抗瞬时抖动）
pub const ERR_FLOOR_RISE: f32 = 1e-5;
/// 双讲检测预热帧数（20ms/帧，25 帧 = 0.5s）：预热期内不冻结，
/// 让自适应先建立回声路径估计，避免基线偏低导致误判。
pub const DTD_WARMUP_FRAMES: u64 = 25;
/// 滤波长度下限 / 上限（毫秒）
pub const MIN_FILTER_MS: u32 = 10;
pub const MAX_FILTER_MS: u32 = 300;

/// 回声消除配置
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AecConfig {
    /// 是否启用
    pub enabled: bool,
    /// 自适应滤波长度（毫秒）：需覆盖「播放→扬声器→空气→麦克风」延迟与回声尾巴
    pub filter_len_ms: u32,
    /// 归一化步长（0~1）
    pub step_size: f32,
    /// 双讲判定比值
    pub dtd_ratio: f32,
}

impl Default for AecConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            // 64ms 覆盖典型桌面场景（扬声器到麦克风延迟通常 < 30ms）
            filter_len_ms: 64,
            step_size: DEFAULT_STEP_SIZE,
            dtd_ratio: DEFAULT_DTD_RATIO,
        }
    }
}

impl AecConfig {
    pub fn validate(&self) -> Result<(), String> {
        if !(MIN_FILTER_MS..=MAX_FILTER_MS).contains(&self.filter_len_ms) {
            return Err(format!(
                "回声消除滤波长度需在 {MIN_FILTER_MS} ~ {MAX_FILTER_MS} 毫秒之间"
            ));
        }
        if !(0.0..=1.0).contains(&self.step_size) {
            return Err("回声消除步长需在 0 ~ 1 之间".into());
        }
        if self.dtd_ratio < 1.0 {
            return Err("双讲判定比值需不小于 1".into());
        }
        Ok(())
    }

    /// 滤波抽头数
    pub fn taps(&self, sample_rate: u32) -> usize {
        let ms = self.filter_len_ms.clamp(MIN_FILTER_MS, MAX_FILTER_MS);
        ((sample_rate as u64 * ms as u64) / 1000).max(1) as usize
    }
}

/// NLMS 回声消除器
pub struct EchoCanceller {
    config: AecConfig,
    sample_rate: u32,
    /// 自适应权值（长度 = taps）
    weights: Vec<f32>,
    /// 远端（参考）样本环形缓冲（长度 = taps）
    history: Vec<f32>,
    /// 下一个写入位置
    cursor: usize,
    /// 平滑后的能量
    mic_power: f32,
    echo_power: f32,
    err_power: f32,
    /// 残差功率历史下限（双讲判据基线）
    err_floor: f32,
    /// 本次处理中是否冻结了自适应
    frozen: bool,
    processing_frames: u64,
}

impl EchoCanceller {
    pub fn new(config: AecConfig, sample_rate: u32) -> Self {
        let taps = config.taps(sample_rate);
        Self {
            config,
            sample_rate,
            weights: vec![0.0; taps],
            history: vec![0.0; taps],
            cursor: 0,
            mic_power: 0.0,
            echo_power: 0.0,
            err_power: 0.0,
            err_floor: 0.0,
            frozen: false,
            processing_frames: 0,
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn taps(&self) -> usize {
        self.weights.len()
    }

    pub fn config(&self) -> &AecConfig {
        &self.config
    }

    /// 是否处于启用状态
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// 上一次处理后是否因双讲冻结了自适应
    pub fn is_frozen(&self) -> bool {
        self.frozen
    }

    /// 已处理的帧数
    pub fn processing_frames(&self) -> u64 {
        self.processing_frames
    }

    /// 回声抑制量（ERLE，dB）：麦克风能量相对残留能量的衰减
    pub fn erle_db(&self) -> f32 {
        if self.err_power <= f32::MIN_POSITIVE || self.mic_power <= f32::MIN_POSITIVE {
            return 0.0;
        }
        10.0 * (self.mic_power / self.err_power).log10()
    }

    /// 重置自适应状态（保留配置）
    pub fn reset(&mut self) {
        self.weights.iter_mut().for_each(|w| *w = 0.0);
        self.history.iter_mut().for_each(|h| *h = 0.0);
        self.cursor = 0;
        self.mic_power = 0.0;
        self.echo_power = 0.0;
        self.err_power = 0.0;
        self.err_floor = 0.0;
        self.frozen = false;
        self.processing_frames = 0;
    }

    /// 环形缓冲中「倒数第 n 个」样本的下标（n=0 为最新写入的样本）
    fn index_back(&self, n: usize) -> usize {
        let taps = self.weights.len();
        (self.cursor + taps - (n % taps)) % taps
    }

    /// 估计回声：`wᵀ·x`
    fn estimate_echo(&self) -> f32 {
        let mut sum = 0.0f32;
        for (n, weight) in self.weights.iter().enumerate() {
            sum += weight * self.history[self.index_back(n)];
        }
        sum
    }

    /// 参考向量能量 `‖x‖²`
    fn reference_energy(&self) -> f32 {
        let mut sum = 0.0f32;
        for n in 0..self.weights.len() {
            let value = self.history[self.index_back(n)];
            sum += value * value;
        }
        sum
    }

    /// 处理一帧音频；长度不一时按较短者处理。
    ///
    /// * `mic_frame`：麦克风采集（含可能的回声）
    /// * `reference_frame`：系统音频回环（远端参考，时间上与麦克风帧对齐）
    pub fn process(&mut self, mic_frame: &[f32], reference_frame: &[f32]) -> Vec<f32> {
        let len = mic_frame.len().min(reference_frame.len());
        if len == 0 {
            return Vec::new();
        }
        self.processing_frames += 1;

        // 帧级双讲判定：使用上一帧累积的能量统计做一次决策。
        // 逐样本判定会因瞬时波动反复冻结/解冻，显著拖慢收敛。
        self.frozen = self.detect_double_talk();

        // 未启用：直通，但仍推进参考缓冲，便于启用瞬间立即收敛
        let mut output = Vec::with_capacity(len);
        for index in 0..len {
            let mic = mic_frame[index];
            let reference = reference_frame[index];

            self.cursor = (self.cursor + 1) % self.weights.len();
            self.history[self.cursor] = reference;

            let echo = self.estimate_echo();
            let error = mic - echo;

            // 能量平滑统计
            self.mic_power =
                DEFAULT_SMOOTHING * self.mic_power + (1.0 - DEFAULT_SMOOTHING) * mic * mic;
            self.echo_power =
                DEFAULT_SMOOTHING * self.echo_power + (1.0 - DEFAULT_SMOOTHING) * echo * echo;
            self.err_power =
                DEFAULT_SMOOTHING * self.err_power + (1.0 - DEFAULT_SMOOTHING) * error * error;

            if self.config.enabled {
                self.adapt(error);
            }

            output.push(if self.config.enabled { error } else { mic });
        }

        output
    }

    /// 双讲判定：「**残差功率突增**」启发式。
    ///
    /// 单讲收敛后残差功率稳定在较低水平；近端一旦开口，残差功率会显著高于
    /// 历史最低水平（`err_floor`），此时冻结自适应，避免近端语音把回声路径带偏。
    /// 相比「麦克风/估计回声能量比」，该判据在收敛中期（估计回声尚未成形）不会误判。
    fn detect_double_talk(&self) -> bool {
        if self.processing_frames <= DTD_WARMUP_FRAMES || self.err_floor <= NOISE_FLOOR {
            return false;
        }
        self.err_power > self.err_floor * self.config.dtd_ratio
    }

    /// NLMS 权值更新（`self.frozen` 为真时仅维护基线、不更新权值）
    fn adapt(&mut self, error: f32) {
        // 维护残差基线：预热期直接跟随，之后立即下探、缓慢回升
        if self.processing_frames <= DTD_WARMUP_FRAMES || self.err_floor <= 0.0 {
            self.err_floor = self.err_power;
        } else if self.err_power < self.err_floor {
            self.err_floor = self.err_power;
        } else {
            self.err_floor += (self.err_power - self.err_floor) * ERR_FLOOR_RISE;
        }

        if self.frozen {
            return;
        }

        let energy = self.reference_energy();
        if energy <= NOISE_FLOOR {
            return;
        }

        let mu = self.config.step_size;
        let gain = mu * error / (1e-6 + energy);
        let taps = self.weights.len();
        for n in 0..taps {
            let index = self.index_back(n);
            self.weights[n] += gain * self.history[index];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: u32 = 16_000;
    const FRAME: usize = 320;

    fn config() -> AecConfig {
        AecConfig {
            enabled: true,
            filter_len_ms: 64,
            step_size: DEFAULT_STEP_SIZE,
            dtd_ratio: DEFAULT_DTD_RATIO,
        }
    }

    /// 确定性伪随机（避免测试依赖 rand）
    struct Lcg(u64);
    impl Lcg {
        fn new(seed: u64) -> Self {
            Self(seed)
        }
        fn next_f32(&mut self) -> f32 {
            self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            // 取高 24 位映射到 [-1, 1)
            let value = ((self.0 >> 40) as f32 / 8_388_608.0) - 1.0;
            value
        }
    }

    /// 合成回声路径：延迟 delay_samples 后接一段衰减冲激响应
    fn echo_path(len: usize, delay_samples: usize) -> Vec<f32> {
        let mut path = vec![0.0f32; len.max(delay_samples + 4)];
        let gains = [0.6f32, -0.35, 0.22, -0.12];
        for (offset, gain) in gains.iter().enumerate() {
            path[delay_samples + offset] = *gain;
        }
        path
    }

    /// 卷积（直接实现，长度小，测试足够）
    fn convolve(signal: &[f32], path: &[f32]) -> Vec<f32> {
        let mut out = vec![0.0f32; signal.len()];
        for n in 0..signal.len() {
            let mut sum = 0.0;
            for (k, tap) in path.iter().enumerate() {
                if n >= k {
                    sum += tap * signal[n - k];
                }
            }
            out[n] = sum;
        }
        out
    }

    fn rms(signal: &[f32]) -> f32 {
        if signal.is_empty() {
            return 0.0;
        }
        (signal.iter().map(|s| s * s).sum::<f32>() / signal.len() as f32).sqrt()
    }

    #[test]
    fn config_maps_filter_length_to_taps() {
        assert_eq!(config().taps(SAMPLE_RATE), 1024, "64ms @16kHz = 1024 抽头");

        let mut short = config();
        short.filter_len_ms = 1;
        assert_eq!(
            short.taps(SAMPLE_RATE),
            (MIN_FILTER_MS as usize * SAMPLE_RATE as usize) / 1000,
            "过小的长度应被夹到下限"
        );
    }

    #[test]
    fn validate_rejects_invalid_values() {
        let mut bad = config();
        bad.filter_len_ms = 5;
        assert!(bad.validate().is_err());

        bad.filter_len_ms = 64;
        bad.step_size = 1.5;
        assert!(bad.validate().is_err());

        bad.step_size = 0.2;
        bad.dtd_ratio = 0.5;
        assert!(bad.validate().is_err());

        bad.dtd_ratio = 4.0;
        assert!(bad.validate().is_ok());
        assert!(AecConfig::default().validate().is_ok());
    }

    /// 合成连续信号流（按帧切分）。
    ///
    /// 注意必须先生成**连续**信号再卷积：若逐帧独立卷积，每帧前 96 个样本的回声
    /// 会因缺少上一帧尾巴而被截断，等效于回声路径每帧跳变，线性滤波器无法收敛。
    fn build_streams(frames: usize, seed: u64) -> (Vec<f32>, Vec<f32>) {
        let mut rng = Lcg::new(seed);
        let reference: Vec<f32> = (0..frames * FRAME).map(|_| 0.4 * rng.next_f32()).collect();
        let path = echo_path(400, 96); // 6ms 延迟 + 4 抽头尾巴
        let echo = convolve(&reference, &path);
        let mic: Vec<f32> = echo
            .iter()
            .map(|sample| sample + 0.0005 * rng.next_f32())
            .collect();
        (reference, mic)
    }

    /// 帧范围能量统计：返回 (麦克风能量, 残差能量)
    fn energy_of(
        canceller: &mut EchoCanceller,
        reference: &[f32],
        mic: &[f32],
        from_frame: usize,
        to_frame: usize,
    ) -> (f64, f64) {
        let mut mic_energy = 0.0f64;
        let mut residual_energy = 0.0f64;
        for frame in from_frame..to_frame {
            let start = frame * FRAME;
            let output = canceller.process(
                &mic[start..start + FRAME],
                &reference[start..start + FRAME],
            );
            assert_eq!(output.len(), FRAME);
            mic_energy += mic[start..start + FRAME]
                .iter()
                .map(|s| (*s as f64).powi(2))
                .sum::<f64>();
            residual_energy += output.iter().map(|s| (*s as f64).powi(2)).sum::<f64>();
        }
        (mic_energy, residual_energy)
    }

    #[test]
    fn cancels_synthetic_echo() {
        let mut canceller = EchoCanceller::new(config(), SAMPLE_RATE);
        let frames = 150; // 3 秒
        let (reference, mic) = build_streams(frames, 42);

        // 前 100 帧用于收敛，后 50 帧（1 秒）测量回声抑制量
        energy_of(&mut canceller, &reference, &mic, 0, 100);
        let (mic_energy, residual_energy) =
            energy_of(&mut canceller, &reference, &mic, 100, frames);

        let erle = 10.0 * (mic_energy / residual_energy).log10();
        assert!(
            erle > 20.0,
            "回声抑制量应超过 20dB，实际 {erle:.1}dB（erle_db={:.1}）",
            canceller.erle_db()
        );
        assert!(!canceller.is_frozen(), "单讲阶段不应冻结自适应");
    }

    #[test]
    fn disabled_canceller_passes_mic_through() {
        let mut passthrough = EchoCanceller::new(
            AecConfig {
                enabled: false,
                ..config()
            },
            SAMPLE_RATE,
        );
        let reference = vec![0.5f32; FRAME];
        let mic = vec![0.2f32; FRAME];

        let output = passthrough.process(&mic, &reference);
        assert_eq!(output, mic, "未启用时应原样直通");
    }

    #[test]
    fn silence_reference_does_not_alter_mic() {
        let mut canceller = EchoCanceller::new(config(), SAMPLE_RATE);
        let mic: Vec<f32> = (0..FRAME).map(|i| (i as f32 * 0.01).sin() * 0.3).collect();
        let reference = vec![0.0f32; FRAME];

        let output = canceller.process(&mic, &reference);
        for (index, value) in output.iter().enumerate() {
            assert!(
                (value - mic[index]).abs() < 1e-6,
                "远端静音且权值为 0 时输出应等于输入"
            );
        }
    }

    #[test]
    fn freezes_adaptation_under_double_talk() {
        let mut canceller = EchoCanceller::new(config(), SAMPLE_RATE);
        let quiet_frames = 100; // 2 秒单讲，用于收敛
        let talk_frames = 60; // 1.2 秒双讲
        let (reference, echo_mic) = build_streams(quiet_frames + talk_frames, 7);

        // 第一阶段：仅回声
        energy_of(&mut canceller, &reference, &echo_mic, 0, quiet_frames);
        assert!(!canceller.is_frozen(), "单讲阶段不应冻结");

        // 第二阶段：叠加近端语音（音量高于回声），自适应应冻结
        let mut frozen_observed = false;
        for frame in quiet_frames..(quiet_frames + talk_frames) {
            let start = frame * FRAME;
            let near_end: Vec<f32> = (0..FRAME)
                .map(|index| 0.8 * ((start + index) as f32 * 0.05).sin())
                .collect();
            let mic: Vec<f32> = echo_mic[start..start + FRAME]
                .iter()
                .zip(near_end.iter())
                .map(|(echo, near)| echo + near)
                .collect();

            let output = canceller.process(&mic, &reference[start..start + FRAME]);

            if canceller.is_frozen() {
                frozen_observed = true;
                // 冻结时近端语音应基本保留（残差能量与近端能量同量级）
                let near_energy: f32 = near_end.iter().map(|s| s * s).sum();
                let residual_energy: f32 = output.iter().map(|s| s * s).sum();
                assert!(
                    residual_energy > 0.3 * near_energy,
                    "双讲时应保留近端语音能量：残差 {residual_energy:.4} vs 近端 {near_energy:.4}"
                );
                break;
            }
        }
        assert!(frozen_observed, "双讲期间应触发自适应冻结");
    }

    #[test]
    fn reset_clears_adaptation_state() {
        let mut canceller = EchoCanceller::new(config(), SAMPLE_RATE);
        let (reference, mic) = build_streams(60, 11);
        energy_of(&mut canceller, &reference, &mic, 0, 60);

        assert!(canceller.processing_frames() > 0);
        assert!(canceller.weights.iter().any(|w| w.abs() > 1e-4), "应已自适应");

        canceller.reset();
        assert_eq!(canceller.processing_frames(), 0);
        assert!(canceller.weights.iter().all(|w| *w == 0.0));
        assert_eq!(canceller.erle_db(), 0.0);
        assert!(!canceller.is_frozen());
    }

    #[test]
    fn unequal_frame_lengths_use_shorter() {
        let mut canceller = EchoCanceller::new(config(), SAMPLE_RATE);
        let mic = vec![0.1f32; 100];
        let reference = vec![0.1f32; 60];

        let output = canceller.process(&mic, &reference);
        assert_eq!(output.len(), 60, "应按较短帧长度处理");
        assert!(canceller.process(&[], &[]).is_empty());
    }

    #[test]
    fn erle_is_zero_before_processing() {
        let canceller = EchoCanceller::new(config(), SAMPLE_RATE);
        assert_eq!(canceller.erle_db(), 0.0);
        assert_eq!(canceller.taps(), 1024);
        assert_eq!(canceller.sample_rate(), SAMPLE_RATE);
        assert!(canceller.is_enabled());
        assert!(!canceller.is_frozen());
    }

    #[test]
    fn rms_helper_sanity() {
        // 保证测试工具函数本身正确（避免断言基于错误基线）
        assert!((rms(&[0.5, -0.5]) - 0.5).abs() < 1e-6);
        assert_eq!(rms(&[]), 0.0);
    }
}
