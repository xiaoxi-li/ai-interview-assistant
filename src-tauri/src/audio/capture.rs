//! 音频采集管线：多声道降混、重采样到 16kHz 单声道、定长分帧与 PCM 编码。
//!
//! 这些纯函数是音频链路的可测试核心（真实设备不可用时用 PCM 回放验证）。

use crate::providers::types::AudioChunk;

/// 目标采样率：ASR 与 VAD 统一使用 16kHz
pub const TARGET_SAMPLE_RATE: u32 = 16_000;
/// 单帧时长（毫秒）
pub const FRAME_MS: u32 = 20;
/// 单帧样本数（16kHz × 20ms）
pub const FRAME_SAMPLES: usize = (TARGET_SAMPLE_RATE as usize * FRAME_MS as usize) / 1000;

/// 多声道降混为单声道
pub fn downmix_to_mono(samples: &[f32], channels: u16) -> Vec<f32> {
    if channels <= 1 {
        return samples.to_vec();
    }
    let channels = channels as usize;
    samples
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / frame.len().max(1) as f32)
        .collect()
}

/// 线性插值重采样到目标采样率
pub fn resample_linear(samples: &[f32], source_rate: u32, target_rate: u32) -> Vec<f32> {
    if source_rate == target_rate || samples.is_empty() || source_rate == 0 {
        return samples.to_vec();
    }

    let ratio = source_rate as f64 / target_rate as f64;
    let out_len = ((samples.len() as f64) / ratio).floor() as usize;
    let mut output = Vec::with_capacity(out_len);

    for index in 0..out_len {
        let position = index as f64 * ratio;
        let left = position.floor() as usize;
        let right = (left + 1).min(samples.len() - 1);
        let fraction = (position - left as f64) as f32;
        output.push(samples[left] * (1.0 - fraction) + samples[right] * fraction);
    }
    output
}

/// 采集数据统一转换为 16kHz 单声道
pub fn normalize_to_16k_mono(samples: &[f32], channels: u16, sample_rate: u32) -> Vec<f32> {
    let mono = downmix_to_mono(samples, channels);
    resample_linear(&mono, sample_rate, TARGET_SAMPLE_RATE)
}

/// 按固定帧长切分（末尾不足一帧的部分会被丢弃，避免产生半帧噪声）
pub fn frame_samples(samples: &[f32]) -> Vec<Vec<f32>> {
    samples
        .chunks(FRAME_SAMPLES)
        .filter(|chunk| chunk.len() == FRAME_SAMPLES)
        .map(|chunk| chunk.to_vec())
        .collect()
}

/// f32 样本量化为 16-bit PCM 小端字节
pub fn samples_to_pcm16(samples: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let value = (clamped * i16::MAX as f32) as i16;
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

/// 构造可供 ASR 消费的音频帧
pub fn to_audio_chunk(samples: &[f32], ts: i64) -> AudioChunk {
    AudioChunk {
        data: samples_to_pcm16(samples),
        sample_rate: TARGET_SAMPLE_RATE,
        channels: 1,
        ts,
    }
}

/// 16-bit PCM 小端字节还原为 f32 样本
pub fn pcm16_to_samples(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(2)
        .map(|pair| i16::from_le_bytes([pair[0], pair[1]]) as f32 / i16::MAX as f32)
        .collect()
}

/// 计算帧的均方根音量（0~1），用于界面电平指示
pub fn rms_level(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f32 = samples.iter().map(|s| s * s).sum();
    (sum / samples.len() as f32).sqrt().clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downmix_averages_channels() {
        // 双声道：左 1.0 / 右 0.0 → 0.5
        let mono = downmix_to_mono(&[1.0, 0.0, 0.5, 0.5], 2);
        assert_eq!(mono.len(), 2);
        assert!((mono[0] - 0.5).abs() < 1e-6);
        assert!((mono[1] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn downmix_passthrough_for_mono() {
        let input = vec![0.1, 0.2, 0.3];
        assert_eq!(downmix_to_mono(&input, 1), input);
    }

    #[test]
    fn resample_48k_to_16k_reduces_length_by_three() {
        let input: Vec<f32> = (0..4800).map(|i| (i as f32) / 4800.0).collect();
        let output = resample_linear(&input, 48_000, 16_000);
        assert_eq!(output.len(), 1600);
        // 首样本保持不变，整体趋势单调递增
        assert!((output[0] - input[0]).abs() < 1e-6);
        assert!(output[1] > output[0]);
    }

    #[test]
    fn resample_is_identity_for_same_rate() {
        let input = vec![0.5, -0.5, 0.25];
        assert_eq!(resample_linear(&input, 16_000, 16_000), input);
    }

    #[test]
    fn resample_handles_empty_input() {
        assert!(resample_linear(&[], 44_100, 16_000).is_empty());
    }

    #[test]
    fn normalize_converts_stereo_44k_to_mono_16k() {
        // 1 秒 44.1kHz 立体声
        let samples: Vec<f32> = (0..88_200).map(|i| ((i as f32) * 0.01).sin()).collect();
        let normalized = normalize_to_16k_mono(&samples, 2, 44_100);
        // 约等于 1 秒的 16kHz 单声道音频
        assert!(
            (normalized.len() as i64 - 16_000).abs() < 100,
            "长度应接近 16000，实际 {}",
            normalized.len()
        );
    }

    #[test]
    fn frame_samples_yields_fixed_size_frames() {
        let samples: Vec<f32> = vec![0.0; FRAME_SAMPLES * 2 + 10];
        let frames = frame_samples(&samples);
        assert_eq!(frames.len(), 2, "不足一帧的尾部应被丢弃");
        assert!(frames.iter().all(|f| f.len() == FRAME_SAMPLES));
        assert_eq!(FRAME_SAMPLES, 320, "16kHz × 20ms = 320 样本");
    }

    #[test]
    fn frame_samples_returns_empty_for_short_input() {
        assert!(frame_samples(&vec![0.0; 10]).is_empty());
    }

    #[test]
    fn pcm16_encoding_clamps_and_is_little_endian() {
        let bytes = samples_to_pcm16(&[1.0, -1.0, 2.0, -2.0]);
        assert_eq!(bytes.len(), 8);
        assert_eq!(&bytes[0..2], &i16::MAX.to_le_bytes());
        assert_eq!(&bytes[2..4], &(-i16::MAX).to_le_bytes());
        // 越界值应被钳制
        assert_eq!(&bytes[4..6], &i16::MAX.to_le_bytes());
        assert_eq!(&bytes[6..8], &(-i16::MAX).to_le_bytes());
    }

    #[test]
    fn to_audio_chunk_carries_target_format() {
        let chunk = to_audio_chunk(&vec![0.0; FRAME_SAMPLES], 1234);
        assert_eq!(chunk.sample_rate, TARGET_SAMPLE_RATE);
        assert_eq!(chunk.channels, 1);
        assert_eq!(chunk.ts, 1234);
        assert_eq!(chunk.data.len(), FRAME_SAMPLES * 2);
    }

    #[test]
    fn pcm16_round_trip_preserves_waveform() {
        let samples: Vec<f32> = vec![0.0, 0.5, -0.5, 1.0, -1.0];
        let restored = pcm16_to_samples(&samples_to_pcm16(&samples));
        assert_eq!(restored.len(), samples.len());
        for (original, actual) in samples.iter().zip(restored.iter()) {
            assert!(
                (original - actual).abs() < 1e-3,
                "PCM 往返误差过大：{original} vs {actual}"
            );
        }
    }

    #[test]
    fn pcm16_decoding_ignores_trailing_odd_byte() {
        let mut bytes = samples_to_pcm16(&[0.5]);
        bytes.push(0x7f); // 残缺字节
        assert_eq!(pcm16_to_samples(&bytes).len(), 1);
    }

    #[test]
    fn rms_level_reflects_amplitude() {
        assert!(rms_level(&[]) < 1e-6);
        assert!(rms_level(&[0.0; 320]) < 1e-6);
        let loud = rms_level(&[0.5; 320]);
        assert!((loud - 0.5).abs() < 1e-6);
        // 超过 1.0 的输入应被钳制
        assert!(rms_level(&[2.0; 320]) <= 1.0);
    }
}
