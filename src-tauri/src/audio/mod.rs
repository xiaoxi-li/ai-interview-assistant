//! 音频子系统。
//!
//! P0 阶段提供设备枚举；音频捕获（麦克风 + 系统音频回环）、VAD、AEC
//! 在 IA-01 / IA-03 / IA-02 任务中逐步实现，均通过 [`AudioCapture`] 抽象隔离平台差异。

pub mod capture;
pub mod devices;
pub mod mic;
pub mod wav;

pub use capture::{
    frame_samples, normalize_to_16k_mono, to_audio_chunk, FRAME_SAMPLES, TARGET_SAMPLE_RATE,
};
pub use devices::{list_by_kind, list_devices, DeviceInfo, DeviceKind};
pub use mic::{find_loopback_device, is_loopback_device_name, loopback_candidates, CpalCapture};

use crate::providers::types::AudioChunk;

/// 跨平台音频捕获抽象（WASAPI loopback / ScreenCaptureKit 差异由此隔离）
pub trait AudioCapture: Send {
    /// 开始捕获（device_id 为空时使用创建时的设备名）
    fn start(&mut self, device_id: &str) -> Result<(), String>;
    /// 停止捕获
    fn stop(&mut self) -> Result<(), String>;
    /// 取出 16kHz 单声道 PCM 帧流（每个实例仅可取出一次）
    fn take_events(&mut self) -> Result<std::sync::mpsc::Receiver<AudioChunk>, String>;
}
