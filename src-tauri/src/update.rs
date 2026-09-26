//! 应用更新检查（P2）。
//!
//! 本模块负责**检查与提示**：拉取更新元数据、比较版本、给出下载地址。
//! 实际的下载与安装由发布流水线配合 `tauri-plugin-updater` 完成
//! （需要仓库 Secrets 中的签名密钥，见 `.github/workflows/release.yml`）。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

/// 更新元数据请求超时
const FETCH_TIMEOUT: Duration = Duration::from_secs(15);

/// 更新检查结果
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
    pub current_version: String,
    /// 远端最新版本（解析失败时为 None）
    pub latest_version: Option<String>,
    /// 是否有可用更新
    pub available: bool,
    pub notes: Option<String>,
    /// 安装包下载地址
    pub url: Option<String>,
    pub message: String,
}

/// 当前平台标识（与 Tauri updater 的 `platforms` 键一致）
pub fn platform_key() -> String {
    let os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else {
        "linux"
    };
    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "unknown"
    };
    format!("{os}-{arch}")
}

/// 解析版本号为数字段（忽略前导 `v` 与预发布后缀）
pub fn parse_version(version: &str) -> Vec<u64> {
    version
        .trim()
        .trim_start_matches('v')
        .split(['-', '+'])
        .next()
        .unwrap_or_default()
        .split('.')
        .map(|part| part.trim().parse::<u64>().unwrap_or(0))
        .collect()
}

/// `latest` 是否比 `current` 更新
pub fn is_newer(latest: &str, current: &str) -> bool {
    let left = parse_version(latest);
    let right = parse_version(current);
    let len = left.len().max(right.len());
    for index in 0..len {
        let a = left.get(index).copied().unwrap_or(0);
        let b = right.get(index).copied().unwrap_or(0);
        if a != b {
            return a > b;
        }
    }
    false
}

/// 解析更新元数据，返回 (版本, 说明, 下载地址)。
///
/// 同时兼容两种格式：
/// * Tauri updater 的 `latest.json`（含 `platforms` 分平台地址）；
/// * 简化格式 `{"version":"1.1.0","notes":"...","url":"..."}`。
pub fn parse_release_metadata(value: &Value, platform: &str) -> Option<(String, Option<String>, Option<String>)> {
    let version = value.get("version")?.as_str()?.trim().to_string();
    if version.is_empty() {
        return None;
    }

    let notes = value
        .get("notes")
        .or_else(|| value.get("body"))
        .and_then(Value::as_str)
        .map(str::to_string);

    let url = value
        .get("platforms")
        .and_then(|platforms| platforms.get(platform))
        .and_then(|entry| entry.get("url"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            value
                .get("url")
                .and_then(Value::as_str)
                .map(str::to_string)
        });

    Some((version, notes, url))
}

/// 拉取并检查更新；`endpoint` 为空时返回可读错误
pub fn check_for_update(endpoint: &str, current_version: &str) -> Result<UpdateCheck, String> {
    let endpoint = endpoint.trim();
    if endpoint.is_empty() {
        return Err("尚未配置更新地址（可在「设置 → 关于与更新」中填写）".into());
    }

    let agent = ureq::AgentBuilder::new().build();
    let response = agent
        .get(endpoint)
        .timeout(FETCH_TIMEOUT)
        .call()
        .map_err(|err| format!("请求更新信息失败：{err}"))?;

    let payload: Value = response
        .into_json()
        .map_err(|err| format!("更新信息格式无效：{err}"))?;

    let platform = platform_key();
    let (latest_version, notes, url) = parse_release_metadata(&payload, &platform)
        .ok_or_else(|| "更新信息缺少 version 字段".to_string())?;

    let available = is_newer(&latest_version, current_version);
    let message = if available {
        format!("发现新版本 {latest_version}（当前 {current_version}）")
    } else {
        format!("已是最新版本（{current_version}）")
    };

    Ok(UpdateCheck {
        current_version: current_version.to_string(),
        latest_version: Some(latest_version),
        available,
        notes,
        url,
        message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    /// 完整读取一个 HTTP 请求（含 body），否则客户端写 body 时会遇到连接重置
    fn read_full_request(stream: &mut std::net::TcpStream) -> String {
        let mut buffer: Vec<u8> = Vec::new();
        let mut chunk = [0u8; 1024];
        loop {
            let read = stream.read(&mut chunk).unwrap_or(0);
            if read == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..read]);
            if buffer.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }
        String::from_utf8_lossy(&buffer).to_string()
    }

    /// 起一个只响应一次的极简 HTTP 服务（请求数必须与调用方一致，否则会阻塞 accept）
    fn spawn_server(body: &'static str, status: u16) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let handle = thread::spawn(move || {
            for stream in listener.incoming().take(1) {
                let Ok(mut stream) = stream else { continue };
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                let _ = read_full_request(&mut stream);
                let response = format!(
                    "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });
        (format!("http://{addr}"), handle)
    }

    #[test]
    fn parses_versions_with_prefix_and_suffix() {
        assert_eq!(parse_version("v1.2.3"), vec![1, 2, 3]);
        assert_eq!(parse_version("1.10.0-beta.1"), vec![1, 10, 0]);
        assert_eq!(parse_version("2"), vec![2]);
        assert_eq!(parse_version(""), vec![0]);
    }

    #[test]
    fn compares_versions_numerically() {
        assert!(is_newer("1.2.3", "1.2.2"));
        assert!(is_newer("1.10.0", "1.9.9"), "应按数值而非字典序比较");
        assert!(is_newer("2.0.0", "1.99.99"));
        assert!(is_newer("1.2.3.1", "1.2.3"), "位数更多且更大应判定为更新");
        assert!(is_newer("v1.3.0", "1.2.9"), "应忽略前导 v");

        assert!(!is_newer("1.2.3", "1.2.3"));
        assert!(!is_newer("1.2.2", "1.2.3"));
        assert!(!is_newer("1.2", "1.2.0"), "补零后相等不应判定为更新");
    }

    #[test]
    fn platform_key_matches_tauri_convention() {
        let key = platform_key();
        assert!(key.contains('-'), "{key}");
        #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
        assert_eq!(key, "windows-x86_64");
    }

    #[test]
    fn parses_tauri_latest_json_format() {
        let value: Value = serde_json::from_str(
            r#"{
                "version": "1.1.0",
                "notes": "修复若干问题",
                "platforms": {
                    "windows-x86_64": { "signature": "sig", "url": "https://example.com/app.msi.zip" },
                    "darwin-aarch64": { "signature": "sig", "url": "https://example.com/app.app.tar.gz" }
                }
            }"#,
        )
        .expect("json");

        let (version, notes, url) =
            parse_release_metadata(&value, "windows-x86_64").expect("metadata");
        assert_eq!(version, "1.1.0");
        assert_eq!(notes.as_deref(), Some("修复若干问题"));
        assert_eq!(url.as_deref(), Some("https://example.com/app.msi.zip"));

        // 平台不匹配时仍能读到版本，但没有下载地址
        let (version, _, url) = parse_release_metadata(&value, "linux-x86_64").expect("metadata");
        assert_eq!(version, "1.1.0");
        assert!(url.is_none());
    }

    #[test]
    fn parses_simple_format_with_direct_url() {
        let value: Value = serde_json::from_str(
            r#"{"version":"2.0.0","body":"大版本更新","url":"https://example.com/setup.exe"}"#,
        )
        .expect("json");

        let (version, notes, url) =
            parse_release_metadata(&value, "windows-x86_64").expect("metadata");
        assert_eq!(version, "2.0.0");
        assert_eq!(notes.as_deref(), Some("大版本更新"));
        assert_eq!(url.as_deref(), Some("https://example.com/setup.exe"));
    }

    #[test]
    fn rejects_metadata_without_version() {
        let value: Value = serde_json::from_str(r#"{"notes":"no version"}"#).expect("json");
        assert!(parse_release_metadata(&value, "windows-x86_64").is_none());
    }

    #[test]
    fn empty_endpoint_returns_actionable_error() {
        let error = check_for_update("   ", "0.1.0").expect_err("应失败");
        assert!(error.contains("尚未配置更新地址"), "{error}");
    }

    #[test]
    fn check_reports_available_update() {
        let (base, handle) = spawn_server(
            r#"{"version":"9.9.9","notes":"测试版本","url":"https://example.com/app.zip"}"#,
            200,
        );
        let result = check_for_update(&base, "0.1.0").expect("check");
        assert!(result.available);
        assert_eq!(result.latest_version.as_deref(), Some("9.9.9"));
        assert_eq!(result.url.as_deref(), Some("https://example.com/app.zip"));
        assert!(result.message.contains("发现新版本 9.9.9"));
        handle.join().ok();
    }

    #[test]
    fn check_reports_up_to_date() {
        let (base, handle) = spawn_server(r#"{"version":"0.1.0"}"#, 200);
        let result = check_for_update(&base, "0.1.0").expect("check");
        assert!(!result.available);
        assert!(result.message.contains("已是最新版本"));
        assert!(result.url.is_none());
        handle.join().ok();
    }

    #[test]
    fn check_reports_http_error() {
        let (base, handle) = spawn_server(r#"{"error":"not found"}"#, 404);
        let error = check_for_update(&base, "0.1.0").expect_err("应失败");
        assert!(error.contains("请求更新信息失败"), "{error}");
        handle.join().ok();
    }

    #[test]
    fn check_reports_malformed_json() {
        let (base, handle) = spawn_server("not-json", 200);
        let error = check_for_update(&base, "0.1.0").expect_err("应失败");
        assert!(error.contains("更新信息格式无效"), "{error}");
        handle.join().ok();
    }
}
