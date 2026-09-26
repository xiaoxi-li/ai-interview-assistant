//! 音频设备枚举（基于 cpal，跨平台抽象 WASAPI / CoreAudio）。
//!
//! * 输入设备 → 麦克风（"我"的声音）
//! * 输出设备 → 系统音频回环来源（面试官的声音，实际捕获由 IA-01 实现）

use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};

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

/// 枚举全部输入/输出设备
pub fn list_devices() -> Vec<DeviceInfo> {
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
