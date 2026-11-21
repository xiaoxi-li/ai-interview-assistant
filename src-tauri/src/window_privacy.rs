//! 窗口防捕获（隐身模式 IA-11）：平台原生实现。
//!
//! **Windows**：`SetWindowDisplayAffinity`
//! * `WDA_EXCLUDEFROMCAPTURE`（Windows 10 2004 / build 19041+）：窗口**完全不出现在**
//!   屏幕共享、截屏与录屏捕获中——腾讯会议 / 钉钉等共享整个屏幕时看不到本窗口；
//! * 旧系统回退 `WDA_MONITOR`：窗口在捕获画面中显示为纯黑块（不暴露内容）；
//! * 恢复显示：`WDA_NONE`。
//!
//! 直接声明 FFI 而不引入 `windows` / `windows-sys` 依赖：`SetWindowDisplayAffinity`
//! 仅需 HWND（isize）与 affinity 标志位，零新增构建负担。
//!
//! **macOS**：沿用 Tauri 的 `set_content_protected`（底层为
//! `NSWindow.sharingType = .none`，共享 / 录屏画面中不显示窗口内容）。

use tauri::WebviewWindow;

#[cfg(windows)]
const WDA_NONE: u32 = 0x00000000;
#[cfg(windows)]
const WDA_MONITOR: u32 = 0x00000001;
/// Windows 10 2004+：窗口完全从捕获（共享 / 截屏 / 录屏）中排除
#[cfg(windows)]
const WDA_EXCLUDEFROMCAPTURE: u32 = 0x00000011;

#[cfg(windows)]
#[link(name = "user32")]
extern "system" {
    fn SetWindowDisplayAffinity(hwnd: *mut core::ffi::c_void, affinity: u32) -> i32;
}

/// 当前平台是否支持窗口级防捕获
pub fn is_supported() -> bool {
    true
}

/// 设置窗口防屏幕共享 / 录屏捕获（`hide = false` 恢复正常显示）。
///
/// 返回 `Err(失败原因)` 供上层反馈；不 panic、不中断调用流程。
pub fn set_hide_from_capture(window: &WebviewWindow, hide: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        // tauri::WebviewWindow::hwnd() 返回 windows crate 的 HWND newtype（内部为 isize）
        let hwnd = window.hwnd().map_err(|err| err.to_string())?;
        let affinity = if hide { WDA_EXCLUDEFROMCAPTURE } else { WDA_NONE };
        if unsafe { SetWindowDisplayAffinity(hwnd.0, affinity) } != 0 {
            return Ok(());
        }
        if !hide {
            return Err("恢复窗口显示失败（SetWindowDisplayAffinity WDA_NONE）".into());
        }
        // Windows 10 < 2004 不支持 EXCLUDEFROMCAPTURE：回退 WDA_MONITOR（捕获中显示黑块）
        if unsafe { SetWindowDisplayAffinity(hwnd.0, WDA_MONITOR) } != 0 {
            tracing::warn!(
                "WDA_EXCLUDEFROMCAPTURE 不可用（需 Windows 10 2004+），已回退 WDA_MONITOR：\
                 屏幕共享画面中本窗口显示为黑色"
            );
            return Ok(());
        }
        Err("SetWindowDisplayAffinity 调用失败".into())
    }

    #[cfg(not(windows))]
    {
        window
            .set_content_protected(hide)
            .map_err(|err| err.to_string())
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn affinity_flags_match_windows_sdk_values() {
        assert_eq!(WDA_NONE, 0);
        assert_eq!(WDA_MONITOR, 1);
        assert_eq!(WDA_EXCLUDEFROMCAPTURE, 0x11);
    }

    #[test]
    fn platform_capture_protection_is_supported() {
        assert!(is_supported());
    }
}
