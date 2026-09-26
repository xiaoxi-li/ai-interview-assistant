//! 音频子系统。
//!
//! P0 阶段提供设备枚举；音频捕获（麦克风 + 系统音频回环）、VAD、AEC
//! 在 IA-01 / IA-03 / IA-02 任务中逐步实现，均通过 [`AudioCapture`] 抽象隔离平台差异。

pub mod devices;

pub use devices::{list_by_kind, list_devices, DeviceInfo, DeviceKind};

use crate::providers::types::AudioChunk;

/// 跨平台音频捕获抽象（WASAPI loopback / ScreenCaptureKit 差异由此隔离）
pub trait AudioCapture: Send {
    /// 开始捕获
    fn start(&mut self, device_id: &str) -> Result<(), String>;
    /// 停止捕获
    fn stop(&mut self) -> Result<(), String>;
    /// 订阅 16kHz 单声道 PCM 帧流
    fn subscribe(&self) -> std::sync::mpsc::Receiver<AudioChunk>;
}
