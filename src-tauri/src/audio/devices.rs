//! 音频设备枚举（基于 cpal，跨平台抽象 WASAPI / CoreAudio）。
//!
//! * 输入设备 → 麦克风（"我"的声音）
//! * 输出设备 → 系统音频回环来源（面试官的声音，实际捕获由 IA-01 实现）

use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// 设备方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceKind {
    Input,
    Output,
}

/// 音频设备信息
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    /// 设备标识（使用设备名，重名时追加序号）
    pub id: String,
    pub name: String,
    pub kind: DeviceKind,
    pub is_default: bool,
}

fn device_name(device: &cpal::Device, index: usize) -> String {
    device
        .name()
        .unwrap_or_else(|_| format!("未命名设备 {}", index + 1))
}

/// 生成唯一 ID：重名设备追加 `#n`
fn unique_id(name: &str, used: &mut Vec<String>) -> String {
    let mut candidate = name.to_string();
    let mut suffix = 2;
    while used.iter().any(|id| id == &candidate) {
        candidate = format!("{name}#{suffix}");
        suffix += 1;
    }
    used.push(candidate.clone());
    candidate
}

/// 真正执行 cpal 枚举（无缓存）
fn enumerate_devices() -> Vec<DeviceInfo> {
    let host = cpal::default_host();
    let default_input = host.default_input_device().map(|d| d.name().ok()).flatten();
    let default_output = host.default_output_device().map(|d| d.name().ok()).flatten();

    let mut devices = Vec::new();
    let mut used_ids: Vec<String> = Vec::new();

    if let Ok(inputs) = host.input_devices() {
        for (index, device) in inputs.enumerate() {
            let name = device_name(&device, index);
            let is_default = default_input.as_deref() == Some(name.as_str());
            let id = unique_id(&name, &mut used_ids);
            devices.push(DeviceInfo {
                id,
                name,
                kind: DeviceKind::Input,
                is_default,
            });
        }
    }

    if let Ok(outputs) = host.output_devices() {
        for (index, device) in outputs.enumerate() {
            let name = device_name(&device, index);
            let is_default = default_output.as_deref() == Some(name.as_str());
            let id = unique_id(&name, &mut used_ids);
            devices.push(DeviceInfo {
                id,
                name,
                kind: DeviceKind::Output,
                is_default,
            });
        }
    }

    devices
}

/// 枚举全部输入/输出设备（输入 = 麦克风，输出 = 系统音频来源）
///
/// 统一走 1.5s 进程内缓存（见 [`list_devices_cached`]），避免页面切换时重复触发
/// cpal / WASAPI 枚举（NFR-17）；需要即时性时由命令层传 `force` 旁路缓存。
pub fn list_devices() -> Vec<DeviceInfo> {
    list_devices_cached(false)
}

/// 设备枚举结果的进程内缓存有效期。
///
/// 页面切换会重复触发设备枚举（设置页与面试辅助页各一次），而 cpal 枚举在 Windows
/// 上要走 WASAPI，单次可达数十至数百毫秒（NFR-17）。1500ms 足以折叠同一次页面切换
/// 内的重复枚举，又不会让「插拔设备」的感知明显滞后。
const DEVICE_CACHE_TTL: Duration = Duration::from_millis(1500);

/// 最近一次设备枚举结果与时间戳
static DEVICE_CACHE: Mutex<Option<(Instant, Vec<DeviceInfo>)>> = Mutex::new(None);

/// 带 TTL 缓存的设备枚举；`force = true` 时强制重新枚举并刷新缓存。
///
/// 「刷新设备列表」一类需要即时性的入口应传 `force`。
pub fn list_devices_cached(force: bool) -> Vec<DeviceInfo> {
    let mut cache = match DEVICE_CACHE.lock() {
        Ok(guard) => guard,
        // 缓存锁中毒时退化为直接枚举，避免把缓存问题放大成功能故障
        Err(_) => return enumerate_devices(),
    };

    if !force {
        if let Some((stamp, devices)) = cache.as_ref() {
            if stamp.elapsed() < DEVICE_CACHE_TTL {
                return devices.clone();
            }
        }
    }

    let devices = enumerate_devices();
    *cache = Some((Instant::now(), devices.clone()));
    devices
}

/// 仅枚举指定方向的设备
pub fn list_by_kind(kind: DeviceKind) -> Vec<DeviceInfo> {
    list_devices()
        .into_iter()
        .filter(|device| device.kind == kind)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_devices_never_panics_and_ids_are_unique() {
        let devices = list_devices();
        let mut ids: Vec<&String> = devices.iter().map(|d| &d.id).collect();
        let total = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), total, "设备 ID 必须唯一");
    }

    #[test]
    fn filtering_by_kind_is_subset_of_all() {
        let all = list_devices();
        let inputs = list_by_kind(DeviceKind::Input);
        let outputs = list_by_kind(DeviceKind::Output);

        assert_eq!(inputs.len() + outputs.len(), all.len());
        assert!(inputs.iter().all(|d| d.kind == DeviceKind::Input));
        assert!(outputs.iter().all(|d| d.kind == DeviceKind::Output));
    }

    #[test]
    fn cached_listing_matches_forced_enumeration() {
        let fresh = list_devices_cached(true);
        let cached = list_devices_cached(false);
        assert_eq!(fresh, cached, "缓存命中结果应与强制枚举结果一致");
    }

    #[test]
    fn unique_id_appends_suffix_for_duplicates() {
        let mut used = Vec::new();
        assert_eq!(unique_id("麦克风", &mut used), "麦克风");
        assert_eq!(unique_id("麦克风", &mut used), "麦克风#2");
        assert_eq!(unique_id("麦克风", &mut used), "麦克风#3");
        assert_eq!(unique_id("扬声器", &mut used), "扬声器");
    }

    #[test]
    fn at_most_one_default_per_kind() {
        let devices = list_devices();
        for kind in [DeviceKind::Input, DeviceKind::Output] {
            let defaults = devices
                .iter()
                .filter(|d| d.kind == kind && d.is_default)
                .count();
            assert!(defaults <= 1, "{kind:?} 方向默认设备不应超过 1 个");
        }
    }
}
