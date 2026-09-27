//! 音频子系统。
//!
//! 设备枚举、采集（麦克风 + 系统音频）、VAD、AEC 均通过 [`AudioCapture`]
//! 抽象隔离平台差异；两路采集共用 [`worker::CaptureWorker`] 的专用线程模型。

pub mod aec;
pub mod capture;
pub mod devices;
#[cfg(windows)]
pub mod loopback;
pub mod mic;
pub mod vad;
pub mod wav;
pub mod worker;

pub use aec::{AecConfig, EchoCanceller};
pub use capture::{
    error_chunk, frame_samples, normalize_to_16k_mono, to_audio_chunk, FRAME_SAMPLES,
    TARGET_SAMPLE_RATE,
};
pub use devices::{list_by_kind, list_devices, DeviceInfo, DeviceKind};
#[cfg(windows)]
pub use loopback::WasapiLoopbackCapture;
pub use mic::{find_loopback_device, is_loopback_device_name, loopback_candidates, CpalCapture};
pub use vad::{
    analyze as analyze_vad_samples, EnergyVad, VadConfig, VadReport, VadSegment, VadSegmenter,
    VoiceActivityDetector,
};

use crate::providers::types::AudioChunk;

/// 跨平台音频捕获抽象（WASAPI loopback / 虚拟回环设备差异由此隔离）
pub trait AudioCapture: Send {
    /// 开始捕获（device_id 为空时使用创建时的设备名）
    fn start(&mut self, device_id: &str) -> Result<(), String>;
    /// 停止捕获
    fn stop(&mut self) -> Result<(), String>;
    /// 取出 16kHz 单声道 PCM 帧流（每个实例仅可取出一次）
    fn take_events(&mut self) -> Result<std::sync::mpsc::Receiver<AudioChunk>, String>;
}

/// 系统音频（面试官）通道的采集实现（IA-14）
///
/// * **Windows**：[`loopback::WasapiLoopbackCapture`] —— 在播放（渲染）设备上以
///   WASAPI loopback 采集，**无需「立体声混音」，也无需安装任何虚拟声卡**；
/// * **其他平台**：[`mic::CpalCapture`] —— macOS 仍依赖 BlackHole / Soundflower
///   等虚拟回环设备（`cpal` 0.15 不具备 CoreAudio 回环能力）。
#[cfg(windows)]
pub type SystemCapture = loopback::WasapiLoopbackCapture;
#[cfg(not(windows))]
pub type SystemCapture = mic::CpalCapture;

/// 全部播放（渲染）设备，系统默认设备排在最前
pub fn render_devices() -> Vec<DeviceInfo> {
    let mut outputs: Vec<DeviceInfo> = list_devices()
        .into_iter()
        .filter(|device| device.kind == DeviceKind::Output)
        .collect();
    outputs.sort_by_key(|device| !device.is_default);
    outputs
}

/// 系统默认播放（渲染）设备名称；无输出设备时返回 `None`
pub fn default_render_device_name() -> Option<String> {
    render_devices().first().map(|device| device.name.clone())
}

/// 可作为「系统音频（面试官）」来源的设备
///
/// * Windows：全部**播放设备**（端点级回环，选中任意一个即可采集其输出）；
/// * 其他平台：名称匹配到的虚拟回环输入设备（BlackHole 等）。
#[cfg(windows)]
pub fn system_source_devices() -> Vec<DeviceInfo> {
    render_devices()
}

/// 可作为「系统音频（面试官）」来源的设备（非 Windows 走虚拟回环设备）
#[cfg(not(windows))]
pub fn system_source_devices() -> Vec<DeviceInfo> {
    loopback_candidates()
}

/// 本机是否具备采集系统音频的能力
pub fn system_audio_available() -> bool {
    !system_source_devices().is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_devices_are_all_outputs_and_default_first() {
        let devices = render_devices();
        assert!(devices
            .iter()
            .all(|device| device.kind == DeviceKind::Output));

        if let Some(position) = devices.iter().position(|device| device.is_default) {
            assert_eq!(position, 0, "系统默认播放设备应排在最前");
        }
    }

    #[test]
    fn default_render_device_matches_first_render_device() {
        assert_eq!(
            default_render_device_name(),
            render_devices().first().map(|device| device.name.clone())
        );
    }

    #[test]
    fn system_audio_availability_matches_source_list() {
        assert_eq!(
            system_audio_available(),
            !system_source_devices().is_empty()
        );
    }
}
