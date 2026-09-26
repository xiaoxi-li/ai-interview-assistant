//! 基于 cpal 的音频采集（麦克风与系统回环设备共用同一实现）。
//!
//! * 麦克风：直接使用输入设备；
//! * 系统音频：Windows 走 WASAPI Loopback（以「立体声混音 / Stereo Mix」形式暴露），
//!   macOS 走 BlackHole / Soundflower 等回环设备；两者都以输入设备形式被 cpal 枚举。
//!
//! 重要：`cpal::Stream` 在 Windows 上不是 `Send`（内部持有 COM 指针），
//! 因此采集流必须由**专用线程持有**，对外只暴露可跨线程传递的句柄 [`CpalCapture`]。

use super::capture::{self, FRAME_MS};
use super::devices::{list_devices, DeviceInfo, DeviceKind};
use super::AudioCapture;
use crate::knowledge::store::now_ms;
use crate::providers::types::AudioChunk;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// 启动超时（等待设备打开）
const START_TIMEOUT: Duration = Duration::from_secs(10);

/// 常见回环设备名称特征（大小写不敏感）
pub const LOOPBACK_NAME_HINTS: [&str; 6] = [
    "stereo mix",
    "立体声混音",
    "loopback",
    "what u hear",
    "blackhole",
    "soundflower",
];

/// 判断设备名是否为系统音频回环设备
pub fn is_loopback_device_name(name: &str) -> bool {
    let lowered = name.to_lowercase();
    LOOPBACK_NAME_HINTS
        .iter()
        .any(|hint| lowered.contains(&hint.to_lowercase()))
}

/// 从设备列表中挑选系统音频回环设备
pub fn find_loopback_device(devices: &[DeviceInfo]) -> Option<&DeviceInfo> {
    devices
        .iter()
        .filter(|device| device.kind == DeviceKind::Input)
        .find(|device| is_loopback_device_name(&device.name))
}

/// 可用的回环设备列表
pub fn loopback_candidates() -> Vec<DeviceInfo> {
    list_devices()
        .into_iter()
        .filter(|device| device.kind == DeviceKind::Input && is_loopback_device_name(&device.name))
        .collect()
}

/// 将新到的交错样本追加进缓冲区，攒满一帧就转换并投递。
///
/// 抽成独立函数以便用 PCM 数据回放做单元测试（无需真实设备）。
/// 返回 false 表示接收端已关闭，应停止采集。
pub fn push_samples(
    buffer: &mut Vec<f32>,
    samples: &[f32],
    channels: u16,
    sample_rate: u32,
    tx: &Sender<AudioChunk>,
) -> bool {
    buffer.extend_from_slice(samples);

    let frame_source_samples =
        (sample_rate as usize * FRAME_MS as usize) / 1000 * channels.max(1) as usize;
    if frame_source_samples == 0 {
        return true;
    }

    while buffer.len() >= frame_source_samples {
        let frame: Vec<f32> = buffer.drain(..frame_source_samples).collect();
        let mono = capture::normalize_to_16k_mono(&frame, channels, sample_rate);
        let chunk = capture::to_audio_chunk(&mono, now_ms());
        if tx.send(chunk).is_err() {
            return false;
        }
    }
    true
}

/// 在调用线程内解析设备并构建输入流（返回的 Stream 必须在同线程保活）
fn build_input_stream(
    device_name: &str,
    tx: Sender<AudioChunk>,
) -> Result<cpal::Stream, String> {
    let host = cpal::default_host();
    let target = device_name.to_lowercase();

    let mut resolved: Option<cpal::Device> = None;
    for device in host
        .input_devices()
        .map_err(|e| format!("枚举输入设备失败：{e}"))?
    {
        if let Ok(name) = device.name() {
            if name.to_lowercase() == target {
                resolved = Some(device);
                break;
            }
        }
    }
    let device = resolved.ok_or_else(|| format!("未找到输入设备：{device_name}"))?;

    let config = device
        .default_input_config()
        .map_err(|e| format!("读取设备默认配置失败：{e}"))?;

    if config.sample_format() != cpal::SampleFormat::F32 {
        return Err(format!(
            "暂不支持的采样格式 {:?}（当前仅实现 f32 输入）",
            config.sample_format()
        ));
    }

    let channels = config.channels();
    let sample_rate = config.sample_rate().0;
    let buffer: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let error_tx = tx.clone();

    let stream = device
        .build_input_stream(
            &config.into(),
            move |data: &[f32], _| {
                if let Ok(mut buffered) = buffer.lock() {
                    push_samples(&mut buffered, data, channels, sample_rate, &tx);
                }
            },
            move |err| {
                eprintln!("音频流错误：{err}");
                let _ = error_tx.send(AudioChunk {
                    data: Vec::new(),
                    sample_rate: 0,
                    channels: 0,
                    ts: -1, // -1 为错误哨兵
                });
            },
            None,
        )
        .map_err(|e| format!("创建输入流失败：{e}"))?;

    stream.play().map_err(|e| format!("启动音频流失败：{e}"))?;
    Ok(stream)
}

/// 采集工作线程句柄
struct Worker {
    stop_tx: Sender<()>,
    events: Option<Receiver<AudioChunk>>,
    join: Option<JoinHandle<()>>,
}

/// 跨线程采集句柄：内部线程持有 cpal Stream，对外仅暴露 Send 字段
#[derive(Default)]
pub struct CpalCapture {
    worker: Option<Worker>,
    device_name: String,
}

impl CpalCapture {
    pub fn new() -> Self {
        Self::default()
    }

    /// 当前设备名
    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    pub fn is_running(&self) -> bool {
        self.worker.is_some()
    }
}

impl AudioCapture for CpalCapture {
    fn start(&mut self, device_id: &str) -> Result<(), String> {
        if self.worker.is_some() {
            return Err("采集已在进行中".into());
        }
        let device_name = if device_id.trim().is_empty() {
            self.device_name.clone()
        } else {
            device_id.to_string()
        };
        if device_name.trim().is_empty() {
            return Err("未指定音频设备".into());
        }

        let (chunk_tx, chunk_rx) = mpsc::channel();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        let thread_device = device_name.clone();

        let join = thread::spawn(move || {
            match build_input_stream(&thread_device, chunk_tx) {
                Ok(stream) => {
                    let _ = ready_tx.send(Ok(()));
                    // 阻塞至停止信号；期间 stream 保持存活，回调持续投递数据
                    let _ = stop_rx.recv();
                    drop(stream);
                }
                Err(err) => {
                    let _ = ready_tx.send(Err(err));
                }
            }
        });

        match ready_rx.recv_timeout(START_TIMEOUT) {
            Ok(Ok(())) => {
                self.device_name = device_name;
                self.worker = Some(Worker {
                    stop_tx,
                    events: Some(chunk_rx),
                    join: Some(join),
                });
                Ok(())
            }
            Ok(Err(err)) => {
                let _ = join.join();
                Err(err)
            }
            Err(_) => Err("启动音频采集超时".into()),
        }
    }

    fn stop(&mut self) -> Result<(), String> {
        if let Some(mut worker) = self.worker.take() {
            let _ = worker.stop_tx.send(());
            if let Some(join) = worker.join.take() {
                let _ = join.join();
            }
        }
        Ok(())
    }

    fn take_events(&mut self) -> Result<Receiver<AudioChunk>, String> {
        self.worker
            .as_mut()
            .and_then(|worker| worker.events.take())
            .ok_or_else(|| "采集尚未启动或事件流已被取出".to_string())
    }
}

impl Drop for CpalCapture {
    fn drop(&mut self) {
        let _ = AudioCapture::stop(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::capture::FRAME_SAMPLES;

    #[test]
    fn detects_loopback_devices_by_name() {
        assert!(is_loopback_device_name("Stereo Mix (Realtek)"));
        assert!(is_loopback_device_name("立体声混音 (Realtek Audio)"));
        assert!(is_loopback_device_name("BlackHole 2ch"));
        assert!(is_loopback_device_name("Soundflower (2ch)"));
        assert!(!is_loopback_device_name("Microphone (USB Audio)"));
        assert!(!is_loopback_device_name("扬声器 (Realtek)"));
    }

    #[test]
    fn finds_loopback_among_devices() {
        let devices = vec![
            DeviceInfo {
                id: "mic".into(),
                name: "Microphone (USB)".into(),
                kind: DeviceKind::Input,
                is_default: true,
            },
            DeviceInfo {
                id: "mix".into(),
                name: "立体声混音 (Realtek)".into(),
                kind: DeviceKind::Input,
                is_default: false,
            },
            DeviceInfo {
                id: "spk".into(),
                name: "扬声器 (Realtek)".into(),
                kind: DeviceKind::Output,
                is_default: true,
            },
        ];

        let found = find_loopback_device(&devices).expect("loopback");
        assert_eq!(found.id, "mix");
    }

    #[test]
    fn returns_none_when_no_loopback_device() {
        let devices = vec![DeviceInfo {
            id: "mic".into(),
            name: "Microphone (USB)".into(),
            kind: DeviceKind::Input,
            is_default: true,
        }];
        assert!(find_loopback_device(&devices).is_none());
    }

    #[test]
    fn push_samples_emits_one_frame_per_20ms_at_48k_stereo() {
        let (tx, rx) = mpsc::channel();
        let mut buffer = Vec::new();

        // 48kHz 立体声：20ms = 960 样本/声道 = 1920 交错样本
        let frame_source_samples = 1920;
        let samples: Vec<f32> = vec![0.25; frame_source_samples * 2];

        assert!(push_samples(&mut buffer, &samples, 2, 48_000, &tx));
        let chunks: Vec<AudioChunk> = rx.try_iter().collect();

        assert_eq!(chunks.len(), 2);
        for chunk in chunks {
            assert_eq!(chunk.sample_rate, 16_000);
            assert_eq!(chunk.channels, 1);
            assert_eq!(chunk.data.len(), FRAME_SAMPLES * 2, "每帧 320 个 16-bit 样本");
        }
        assert!(buffer.is_empty(), "整帧消费后不应有残留");
    }

    #[test]
    fn push_samples_keeps_partial_frame_buffered() {
        let (tx, rx) = mpsc::channel();
        let mut buffer = Vec::new();

        // 只喂半帧（48kHz 单声道 20ms = 960 样本）
        assert!(push_samples(&mut buffer, &vec![0.0; 480], 1, 48_000, &tx));
        assert!(rx.try_iter().next().is_none(), "不足一帧不应产出");
        assert_eq!(buffer.len(), 480);

        assert!(push_samples(&mut buffer, &vec![0.0; 480], 1, 48_000, &tx));
        assert_eq!(rx.try_iter().count(), 1);
        assert!(buffer.is_empty());
    }

    #[test]
    fn push_samples_stops_when_receiver_dropped() {
        let (tx, rx) = mpsc::channel();
        drop(rx);
        let mut buffer = Vec::new();

        assert!(
            !push_samples(&mut buffer, &vec![0.0; 1920], 2, 48_000, &tx),
            "接收端关闭时应返回 false 以停止采集"
        );
    }

    #[test]
    fn push_samples_is_safe_for_empty_input() {
        let (tx, _rx) = mpsc::channel();
        let mut buffer = Vec::new();
        assert!(push_samples(&mut buffer, &[], 1, 16_000, &tx));
        assert!(buffer.is_empty());
    }

    #[test]
    fn capture_starts_idle_and_rejects_unknown_device() {
        let mut capture = CpalCapture::new();
        assert!(!capture.is_running());
        assert!(capture.take_events().is_err(), "未启动时不应有事件流");

        // 指定一个不存在的设备应返回描述性错误（不 panic）
        let error = capture
            .start("__definitely-not-a-device__")
            .expect_err("应失败");
        assert!(error.contains("未找到输入设备"), "实际错误：{error}");
        assert!(!capture.is_running());
    }

    #[test]
    fn stop_is_idempotent_when_not_running() {
        let mut capture = CpalCapture::new();
        assert!(capture.stop().is_ok());
        assert!(capture.stop().is_ok());
    }
}
