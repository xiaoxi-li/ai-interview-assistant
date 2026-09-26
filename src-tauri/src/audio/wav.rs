//! WAV 读取：用于音频链路集成测试（PCM 文件回放）与调试。
//!
//! 仅支持未压缩的 16-bit PCM WAV，足以覆盖回放测试场景。

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// WAV 解析结果
#[derive(Debug, Clone, PartialEq)]
pub struct WavData {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    /// 归一化到 -1.0 ~ 1.0 的交错样本
    pub samples: Vec<f32>,
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

/// 解析 16-bit PCM WAV
pub fn read_wav(path: &Path) -> Result<WavData, String> {
    let mut file = File::open(path).map_err(|e| format!("打开 WAV 失败：{e}"))?;
    let mut header = [0u8; 12];
    file.read_exact(&mut header)
        .map_err(|e| format!("读取 WAV 头失败：{e}"))?;

    if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
        return Err("不是合法的 WAV 文件".into());
    }

    let mut sample_rate = 0u32;
    let mut channels = 0u16;
    let mut bits_per_sample = 0u16;
    let mut data: Option<Vec<f32>> = None;

    // 遍历 chunk，兼容 fmt/data 顺序与附加 chunk
    loop {
        let mut chunk_header = [0u8; 8];
        if file.read_exact(&mut chunk_header).is_err() {
            break;
        }
        let chunk_id = &chunk_header[0..4];
        let chunk_size = read_u32(&chunk_header, 4) as usize;
        let mut payload = vec![0u8; chunk_size];
        if file.read_exact(&mut payload).is_err() {
            break;
        }

        match chunk_id {
            b"fmt " => {
                if payload.len() < 16 {
                    return Err("fmt chunk 长度不足".into());
                }
                let audio_format = read_u16(&payload, 0);
                if audio_format != 1 {
                    return Err(format!("仅支持 PCM 编码，实际 format = {audio_format}"));
                }
                channels = read_u16(&payload, 2);
                sample_rate = read_u32(&payload, 4);
                bits_per_sample = read_u16(&payload, 14);
                if bits_per_sample != 16 {
                    return Err(format!("仅支持 16-bit 采样，实际 {bits_per_sample}"));
                }
            }
            b"data" => {
                let mut samples = Vec::with_capacity(payload.len() / 2);
                for pair in payload.chunks_exact(2) {
                    let value = i16::from_le_bytes([pair[0], pair[1]]);
                    samples.push(value as f32 / i16::MAX as f32);
                }
                data = Some(samples);
            }
            _ => {
                // 跳过不关心的 chunk；若长度为奇数需补齐填充字节
                if chunk_size % 2 == 1 {
                    let _ = file.seek(SeekFrom::Current(1));
                }
            }
        }

        if chunk_size % 2 == 1 && chunk_id != b"data" {
            let _ = file.seek(SeekFrom::Current(1));
        }
    }

    let samples = data.ok_or_else(|| "WAV 中缺少 data chunk".to_string())?;
    if channels == 0 || sample_rate == 0 {
        return Err("WAV 缺少有效的 fmt 信息".into());
    }

    Ok(WavData {
        sample_rate,
        channels,
        bits_per_sample,
        samples,
    })
}

/// 读取 WAV 并转换为 16kHz 单声道样本（回放测试入口）
pub fn read_wav_as_16k_mono(path: &Path) -> Result<Vec<f32>, String> {
    let wav = read_wav(path)?;
    Ok(super::capture::normalize_to_16k_mono(
        &wav.samples,
        wav.channels,
        wav.sample_rate,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::capture::{frame_samples, FRAME_SAMPLES};
    use std::f32::consts::PI;

    /// 生成 16-bit PCM WAV 文件（测试用，避免引入音频资产）
    fn write_pcm16_wav(
        path: &Path,
        sample_rate: u32,
        channels: u16,
        samples: &[i16],
    ) {
        let bits = 16u16;
        let block_align = channels * bits / 8;
        let byte_rate = sample_rate * block_align as u32;
        let data_len = (samples.len() * 2) as u32;

        let mut bytes: Vec<u8> = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
        bytes.extend_from_slice(&channels.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&byte_rate.to_le_bytes());
        bytes.extend_from_slice(&block_align.to_le_bytes());
        bytes.extend_from_slice(&bits.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }

        std::fs::write(path, bytes).expect("write wav");
    }

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "aia-wav-{tag}-{}-{}",
            std::process::id(),
            crate::knowledge::store::now_ms()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn sine(sample_rate: u32, seconds: f32, freq: f32) -> Vec<i16> {
        (0..(sample_rate as f32 * seconds) as usize)
            .map(|i| {
                let value = (2.0 * PI * freq * i as f32 / sample_rate as f32).sin();
                (value * 0.5 * i16::MAX as f32) as i16
            })
            .collect()
    }

    #[test]
    fn reads_mono_16k_pcm_wav() {
        let dir = temp_dir("mono");
        let file = dir.join("mono.wav");
        write_pcm16_wav(&file, 16_000, 1, &sine(16_000, 0.1, 440.0));

        let wav = read_wav(&file).expect("read");
        assert_eq!(wav.sample_rate, 16_000);
        assert_eq!(wav.channels, 1);
        assert_eq!(wav.bits_per_sample, 16);
        assert_eq!(wav.samples.len(), 1600);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn stereo_44k_wav_is_normalized_for_replay() {
        let dir = temp_dir("stereo");
        let file = dir.join("stereo.wav");
        // 0.5 秒 44.1kHz 立体声
        let mono = sine(44_100, 0.5, 220.0);
        let interleaved: Vec<i16> = mono.iter().flat_map(|s| [*s, *s]).collect();
        write_pcm16_wav(&file, 44_100, 2, &interleaved);

        let samples = read_wav_as_16k_mono(&file).expect("replay");
        assert!(
            (samples.len() as i64 - 8_000).abs() < 100,
            "0.5 秒应重采样为约 8000 个样本，实际 {}",
            samples.len()
        );

        // 重采样后可切出完整帧，供 ASR/VAD 消费
        let frames = frame_samples(&samples);
        assert!(!frames.is_empty());
        assert!(frames.iter().all(|f| f.len() == FRAME_SAMPLES));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rejects_non_riff_file() {
        let dir = temp_dir("bad");
        let file = dir.join("bad.wav");
        std::fs::write(&file, b"not a wav file at all").expect("write");

        assert!(read_wav(&file).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rejects_non_pcm_format() {
        let dir = temp_dir("float");
        let file = dir.join("float.wav");
        let mut bytes: Vec<u8> = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&36u32.to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&3u16.to_le_bytes()); // IEEE float
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&16_000u32.to_le_bytes());
        bytes.extend_from_slice(&32_000u32.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&0u32.to_le_bytes());
        std::fs::write(&file, bytes).expect("write");

        let error = read_wav(&file).expect_err("should reject");
        assert!(error.contains("PCM"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
