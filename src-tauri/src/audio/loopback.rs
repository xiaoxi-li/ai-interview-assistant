//! Windows 系统音频采集：**WASAPI 端点级回环**（IA-14）。
//!
//! 在**播放（渲染）端点**上以 loopback 模式打开采集流，因此：
//!
//! * 无需在「声音设置 → 录制」里启用「立体声混音」，也无需安装任何虚拟声卡；
//! * 取得的是该播放设备上**所有应用混合后**的输出（会议软件、播放器、浏览器…）；
//! * 相比麦克风回采，回环流本身即是扬声器实际播放内容，作为 AEC 参考更准确。
//!
//! `wasapi` 在 `initialize_client` 内部按「客户端方向为 `Render` + 请求方向为 `Capture`
//! + 共享模式」自动附加 `AUDCLNT_STREAMFLAGS_LOOPBACK`，调用方无需拼装原始标志位。
//!
//! 与 [`CpalCapture`](super::mic::CpalCapture) 一致：COM 对象不是 `Send`，采集流由专用
//! 线程持有（[`CaptureWorker`]），对外只暴露可跨线程传递的句柄。

use super::mic::push_samples;
use super::worker::{CaptureWorker, Ready};
use super::AudioCapture;
use crate::providers::types::AudioChunk;
use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, Sender};
use wasapi::{
    initialize_mta, Device, DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat,
};

/// 请求的采集格式：开启 `autoconvert` 后由音频引擎负责与设备混音格式互转
const CAPTURE_SAMPLE_RATE: u32 = 48_000;
/// 回环目标为单声道内容，按立体声请求后统一降混
const CAPTURE_CHANNELS: u16 = 2;
/// 设备缓冲时长（单位 100ns）：20ms
const BUFFER_DURATION_HNS: i64 = 200_000;
/// 事件等待超时（毫秒）
///
/// 播放端点空闲（无任何应用出声）时 WASAPI 可能不再触发事件，故超时按
/// 「暂时无数据」处理并继续轮询，而不是判定为错误。
const EVENT_TIMEOUT_MS: u32 = 200;

/// 跨线程回环采集句柄：内部线程持有 WASAPI 采集流
#[derive(Default)]
pub struct WasapiLoopbackCapture {
    worker: Option<CaptureWorker>,
    device_name: String,
}

impl WasapiLoopbackCapture {
    pub fn new() -> Self {
        Self::default()
    }

    /// 当前播放设备名（尚未启动且未指定时为空，表示使用系统默认播放设备）
    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    pub fn is_running(&self) -> bool {
        self.worker.is_some()
    }
}

impl AudioCapture for WasapiLoopbackCapture {
    fn start(&mut self, device_id: &str) -> Result<(), String> {
        if self.worker.is_some() {
            return Err("采集已在进行中".into());
        }

        // 设备为空时依次回退：调用方显式指定 → 上次使用的设备 → 系统默认播放设备
        let device_name = if !device_id.trim().is_empty() {
            device_id.trim().to_string()
        } else if !self.device_name.trim().is_empty() {
            self.device_name.clone()
        } else {
            super::default_render_device_name().unwrap_or_default()
        };

        let thread_device = device_name.clone();
        let worker = CaptureWorker::spawn(move |ready, tx, stop_rx| {
            run_loopback(&thread_device, ready, tx, stop_rx)
        })?;

        self.device_name = device_name;
        self.worker = Some(worker);
        Ok(())
    }

    fn stop(&mut self) -> Result<(), String> {
        if let Some(mut worker) = self.worker.take() {
            worker.stop();
        }
        Ok(())
    }

    fn take_events(&mut self) -> Result<Receiver<AudioChunk>, String> {
        match self.worker.as_mut() {
            Some(worker) => worker.take_events(),
            None => Err("采集尚未启动或事件流已被取出".to_string()),
        }
    }
}

/// 解析要采集的播放设备：为空时取系统默认播放设备
fn resolve_render_device(enumerator: &DeviceEnumerator, requested: &str) -> Result<Device, String> {
    if requested.trim().is_empty() {
        return enumerator
            .get_default_device(&Direction::Render)
            .map_err(|e| format!("未找到系统默认播放设备：{e}"));
    }

    enumerator
        .get_device_collection(&Direction::Render)
        .and_then(|collection| collection.get_device_with_name(requested))
        .map_err(|e| format!("未找到播放设备「{requested}」：{e}"))
}

/// 把已读取的原始字节按小端 f32 解出样本（不足 4 字节的尾部保留到下一轮）
pub fn drain_f32_le(bytes: &mut VecDeque<u8>) -> Vec<f32> {
    let mut samples = Vec::with_capacity(bytes.len() / 4);
    while bytes.len() >= 4 {
        let mut raw = [0u8; 4];
        for slot in raw.iter_mut() {
            if let Some(byte) = bytes.pop_front() {
                *slot = byte;
            }
        }
        samples.push(f32::from_le_bytes(raw));
    }
    samples
}

/// 采集线程主体：打开回环流 → 循环投递 16kHz 单声道帧
fn run_loopback(
    requested_device: &str,
    ready: Ready,
    tx: Sender<AudioChunk>,
    stop_rx: Receiver<()>,
) -> Result<(), String> {
    // 采集线程首次使用 COM：重复初始化会返回错误，可忽略
    let _ = initialize_mta();

    let enumerator =
        DeviceEnumerator::new().map_err(|e| format!("初始化音频设备枚举失败：{e}"))?;
    let device = resolve_render_device(&enumerator, requested_device)?;
    let friendly = device
        .get_friendlyname()
        .unwrap_or_else(|_| requested_device.to_string());
    let mut client = device
        .get_iaudioclient()
        .map_err(|e| format!("获取音频客户端失败（{friendly}）：{e}"))?;

    let format = WaveFormat::new(
        32,
        32,
        &SampleType::Float,
        CAPTURE_SAMPLE_RATE as usize,
        CAPTURE_CHANNELS as usize,
        None,
    );
    let mode = StreamMode::EventsShared {
        autoconvert: true,
        buffer_duration_hns: BUFFER_DURATION_HNS,
    };
    // 在播放端点上请求 Capture 方向 → wasapi 自动附加 AUDCLNT_STREAMFLAGS_LOOPBACK
    client
        .initialize_client(&format, &Direction::Capture, &mode)
        .map_err(|e| format!("初始化系统音频回环失败（播放设备「{friendly}」）：{e}"))?;

    let event = client
        .set_get_eventhandle()
        .map_err(|e| format!("创建音频事件失败：{e}"))?;
    let capture = client
        .get_audiocaptureclient()
        .map_err(|e| format!("获取采集客户端失败：{e}"))?;
    client
        .start_stream()
        .map_err(|e| format!("启动系统音频回环失败（播放设备「{friendly}」）：{e}"))?;

    ready.ok();

    let mut bytes: VecDeque<u8> = VecDeque::new();
    let mut buffer: Vec<f32> = Vec::new();

    while stop_rx.try_recv().is_err() {
        let pending = match capture.get_next_packet_size() {
            Ok(Some(frames)) => frames,
            Ok(None) => 0,
            Err(err) => {
                tracing::warn!("系统音频回环读取失败：{err}");
                let _ = tx.send(crate::audio::capture::error_chunk());
                break;
            }
        };

        if pending > 0 {
            if let Err(err) = capture.read_from_device_to_deque(&mut bytes) {
                tracing::warn!("系统音频回环读取失败：{err}");
                let _ = tx.send(crate::audio::capture::error_chunk());
                break;
            }
            let samples = drain_f32_le(&mut bytes);
            if !push_samples(
                &mut buffer,
                &samples,
                CAPTURE_CHANNELS,
                CAPTURE_SAMPLE_RATE,
                &tx,
            ) {
                break; // 接收端已关闭
            }
        }

        // 空闲时事件不会触发：超时继续轮询
        let _ = event.wait_for_event(EVENT_TIMEOUT_MS);
    }

    let _ = client.stop_stream();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drain_f32_le_decodes_little_endian_samples() {
        let mut bytes = VecDeque::new();
        for value in [0.5f32, -0.25f32] {
            bytes.extend(value.to_le_bytes());
        }
        assert_eq!(drain_f32_le(&mut bytes), vec![0.5, -0.25]);
        assert!(bytes.is_empty());
    }

    #[test]
    fn drain_f32_le_keeps_incomplete_tail() {
        let mut bytes = VecDeque::new();
        bytes.extend(1.0f32.to_le_bytes());
        bytes.extend([0x01, 0x02, 0x03]); // 残缺样本
        assert_eq!(drain_f32_le(&mut bytes), vec![1.0]);
        assert_eq!(bytes.len(), 3, "不足 4 字节的尾部应保留给下一轮");
    }

    #[test]
    fn drain_f32_le_is_safe_for_empty_input() {
        let mut bytes = VecDeque::new();
        assert!(drain_f32_le(&mut bytes).is_empty());
    }

    #[test]
    fn capture_starts_idle_and_rejects_unknown_render_device() {
        let mut capture = WasapiLoopbackCapture::new();
        assert!(!capture.is_running());
        assert!(capture.take_events().is_err(), "未启动时不应有事件流");

        let error = capture
            .start("__definitely-not-a-playback-device__")
            .expect_err("不存在的播放设备应失败");
        assert!(
            error.contains("未找到播放设备") || error.contains("未找到系统默认播放设备"),
            "实际错误：{error}"
        );
        assert!(!capture.is_running());
    }

    #[test]
    fn stop_is_idempotent_when_not_running() {
        let mut capture = WasapiLoopbackCapture::new();
        assert!(capture.stop().is_ok());
        assert!(capture.stop().is_ok());
    }
}
