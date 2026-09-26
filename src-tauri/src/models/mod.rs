//! 模型管理器：本地模型（嵌入 / VAD / 本地 ASR）的清单、下载与校验。
//!
//! 设计要点：
//! * 安装包不包含模型（保持 < 20MB），模型按需下载到应用数据目录；
//! * 下载支持断点续传（HTTP Range），完成后做 SHA256 校验；
//! * 校验失败的半成品保留为 `.part` 文件，便于重新续传或手动清理。

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// 模型类别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelKind {
    Embedding,
    Vad,
    Asr,
}

/// 模型清单条目
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSpec {
    pub id: String,
    pub name: String,
    pub kind: ModelKind,
    /// 本地文件名
    pub file_name: String,
    /// 相对镜像根目录的路径
    pub path: String,
    /// 期望的 SHA256（小写十六进制）；为空表示跳过校验
    pub sha256: String,
    /// 期望大小（字节，0 表示未知）
    pub size_bytes: u64,
    pub description: String,
    /// 本地 ASR 所需（选择本地模式时下载）
    pub required_for_local_asr: bool,
}

/// 模型安装状态
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    #[serde(flatten)]
    pub spec: ModelSpec,
    pub installed: bool,
    pub resolved_path: Option<String>,
    /// 已下载占比（存在 .part 文件时 > 0）
    pub partial_bytes: u64,
}

/// 镜像根地址（仅读取环境变量，便于内网/离线部署）
pub fn mirror_base() -> String {
    std::env::var("AIA_MODEL_MIRROR").unwrap_or_default()
}

/// 解析镜像根地址：配置值优先，其次环境变量 `AIA_MODEL_MIRROR`，否则为空（未配置）。
///
/// 空串表示「未配置镜像源」，下载前需据此给出可操作的错误提示，而非静默失败。
pub fn resolve_mirror(configured: Option<&str>) -> String {
    if let Some(url) = configured {
        let trimmed = url.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    std::env::var("AIA_MODEL_MIRROR").unwrap_or_default()
}

/// 镜像连通性探测结果（HEAD 请求，不下载正文）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MirrorProbe {
    /// 是否可达（2xx）
    pub ok: bool,
    /// HTTP 状态码
    pub status: Option<u16>,
    /// 响应体大小（Content-Length）
    pub content_length: Option<u64>,
    /// 是否支持断点续传（Accept-Ranges: bytes）
    pub supports_range: bool,
    /// 可读说明（失败原因等）
    pub message: String,
}

/// 用 HEAD 请求探测某个模型文件是否可访问（不下载正文，避免误拉取大文件）。
pub fn probe_url(base_url: &str, path: &str) -> MirrorProbe {
    if base_url.trim().is_empty() {
        return MirrorProbe {
            ok: false,
            status: None,
            content_length: None,
            supports_range: false,
            message: "镜像源未配置".to_string(),
        };
    }

    let url = format!("{}/{}", base_url.trim_end_matches('/'), path.trim_start_matches('/'));
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(15))
        .build();

    match agent.head(&url).call() {
        Ok(resp) => {
            let status = resp.status();
            let content_length = resp
                .header("Content-Length")
                .and_then(|v| v.parse::<u64>().ok());
            let supports_range = resp
                .header("Accept-Ranges")
                .map(|v| v.to_ascii_lowercase().contains("bytes"))
                .unwrap_or(false);
            let ok = (200..300).contains(&status);
            MirrorProbe {
                ok,
                status: Some(status),
                content_length,
                supports_range,
                message: if ok {
                    "可达".to_string()
                } else {
                    format!("HTTP {status}")
                },
            }
        }
        Err(err) => MirrorProbe {
            ok: false,
            status: None,
            content_length: None,
            supports_range: false,
            message: format!("不可达：{err}"),
        },
    }
}

/// 内置模型清单
pub fn catalog() -> Vec<ModelSpec> {
    vec![
        ModelSpec {
            id: "embedding-minilm-zh".into(),
            name: "中文嵌入模型（MiniLM）".into(),
            kind: ModelKind::Embedding,
            file_name: "embedding-minilm-zh.onnx".into(),
            path: "embedding/minilm-zh.onnx".into(),
            sha256: String::new(),
            size_bytes: 0,
            description: "本地向量检索使用的轻量嵌入模型，约 90MB".into(),
            required_for_local_asr: false,
        },
        ModelSpec {
            id: "vad-silero".into(),
            name: "Silero VAD".into(),
            kind: ModelKind::Vad,
            file_name: "silero_vad.onnx".into(),
            path: "vad/silero_vad.onnx".into(),
            sha256: String::new(),
            size_bytes: 0,
            description: "语音活动检测模型，用于静音过滤与自动分段，约 2MB".into(),
            required_for_local_asr: false,
        },
        ModelSpec {
            id: "asr-paraformer-zh".into(),
            name: "本地 ASR（Paraformer 中文）".into(),
            kind: ModelKind::Asr,
            file_name: "sherpa-onnx-paraformer-zh.tar.bz2".into(),
            path: "asr/sherpa-onnx-paraformer-zh.tar.bz2".into(),
            sha256: String::new(),
            size_bytes: 0,
            description: "本地语音识别模型，仅在选择本地 ASR 模式时需要，约 200MB".into(),
            required_for_local_asr: true,
        },
    ]
}

/// 按 ID 查找清单条目
pub fn find_spec(id: &str) -> Option<ModelSpec> {
    catalog().into_iter().find(|spec| spec.id == id)
}

/// 剔除路径中的不安全片段，防止目录穿越
fn safe_relative_path(path: &str) -> Result<PathBuf, String> {
    let candidate = PathBuf::from(path);
    let mut safe = PathBuf::new();
    for component in candidate.components() {
        match component {
            std::path::Component::Normal(part) => safe.push(part),
            _ => return Err(format!("非法模型路径：{path}")),
        }
    }
    if safe.as_os_str().is_empty() {
        return Err("模型路径不能为空".into());
    }
    Ok(safe)
}

/// 模型在本地磁盘上的目标路径
pub fn model_file_path(models_dir: &Path, spec: &ModelSpec) -> Result<PathBuf, String> {
    Ok(models_dir.join(safe_relative_path(&spec.file_name)?))
}

/// 断点续传使用的临时文件路径
pub fn part_file_path(target: &Path) -> PathBuf {
    let mut name = target.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    target.with_file_name(name)
}

/// 计算文件 SHA256（小写十六进制）
pub fn file_sha256(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| format!("打开文件失败：{e}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// 校验文件 SHA256 是否匹配期望值（期望为空时跳过校验）
pub fn verify_file(path: &Path, expected: &str) -> Result<bool, String> {
    if expected.trim().is_empty() {
        return Ok(path.exists());
    }
    if !path.exists() {
        return Ok(false);
    }
    let actual = file_sha256(path)?;
    Ok(actual.eq_ignore_ascii_case(expected))
}

/// 查询清单中每个模型的安装状态
pub fn list_status(models_dir: &Path) -> Vec<ModelStatus> {
    catalog()
        .into_iter()
        .map(|spec| {
            let target = model_file_path(models_dir, &spec).ok();
            let part = target.as_ref().map(|p| part_file_path(p));
            let installed = target.as_ref().map(|p| p.exists()).unwrap_or(false);
            let partial_bytes = part
                .as_ref()
                .and_then(|p| std::fs::metadata(p).ok())
                .map(|m| m.len())
                .unwrap_or(0);
            ModelStatus {
                resolved_path: target.as_ref().map(|p| p.display().to_string()),
                installed,
                partial_bytes,
                spec,
            }
        })
        .collect()
}

/// 下载进度回调：(已下载字节, 总字节 Option)
pub type ProgressFn<'a> = &'a mut dyn FnMut(u64, Option<u64>);

/// 下载模型文件，支持断点续传与 SHA256 校验。
///
/// 成功后返回最终文件路径；校验失败会保留 `.part` 文件并返回错误。
pub fn download_model(
    models_dir: &Path,
    spec: &ModelSpec,
    base_url: &str,
    on_progress: ProgressFn<'_>,
) -> Result<PathBuf, String> {
    let target = model_file_path(models_dir, spec)?;
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let part = part_file_path(&target);

    let mut offset = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
    let url = format!("{}/{}", base_url.trim_end_matches('/'), spec.path);

    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(600))
        .build();
    let mut request = agent.get(&url);
    if offset > 0 {
        request = request.set("Range", &format!("bytes={offset}-"));
    }

    let response = request.call().map_err(|e| format!("下载失败：{e}"))?;
    let status = response.status();

    // 服务端不支持续传（返回 200）时从头下载
    if offset > 0 && status != 206 {
        let _ = std::fs::remove_file(&part);
        offset = 0;
    }

    let total = response
        .header("Content-Length")
        .and_then(|v| v.parse::<u64>().ok())
        .map(|len| len + offset);

    let mut reader = response.into_reader();
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&part)
        .map_err(|e| format!("创建临时文件失败：{e}"))?;
    let mut writer = BufWriter::new(file);

    let mut downloaded = offset;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer).map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        writer.write_all(&buffer[..read]).map_err(|e| e.to_string())?;
        downloaded += read as u64;
        on_progress(downloaded, total);
    }
    writer.flush().map_err(|e| e.to_string())?;
    drop(writer);

    // 校验
    if !spec.sha256.trim().is_empty() {
        let actual = file_sha256(&part)?;
        if !actual.eq_ignore_ascii_case(&spec.sha256) {
            return Err(format!(
                "SHA256 校验失败（期望 {}，实际 {actual}），已保留 .part 文件可重新续传",
                spec.sha256
            ));
        }
    }

    std::fs::rename(&part, &target).map_err(|e| format!("落盘失败：{e}"))?;
    Ok(target)
}

/// 删除已安装模型（同时清理 .part 残留）
pub fn delete_model(models_dir: &Path, spec: &ModelSpec) -> Result<bool, String> {
    let target = model_file_path(models_dir, spec)?;
    let part = part_file_path(&target);
    let mut removed = false;
    for path in [target, part] {
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
            removed = true;
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    fn temp_dir(tag: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "aia-models-{tag}-{}-{}",
            std::process::id(),
            crate::knowledge::store::now_ms()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn sha256_of(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
    }

    fn spec_for(payload: &[u8]) -> ModelSpec {
        ModelSpec {
            id: "test-model".into(),
            name: "测试模型".into(),
            kind: ModelKind::Embedding,
            file_name: "test-model.bin".into(),
            path: "test/model.bin".into(),
            sha256: sha256_of(payload),
            size_bytes: payload.len() as u64,
            description: "单测用".into(),
            required_for_local_asr: false,
        }
    }

    /// 极简文件服务：支持 Range 续传。
    /// 注意：处理请求数必须与调用方实际请求数一致（此处单次下载仅 1 个请求），
    /// 否则线程会阻塞在 accept 上导致 join 挂起。
    fn spawn_file_server(payload: Vec<u8>) -> (String, thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let handle = thread::spawn(move || {
            let mut seen_requests = Vec::new();
            for stream in listener.incoming().take(1) {
                let Ok(mut stream) = stream else { continue };
                let mut buffer = [0u8; 2048];
                let read = stream.read(&mut buffer).unwrap_or(0);
                let request = String::from_utf8_lossy(&buffer[..read]).to_string();
                seen_requests.push(request.clone());

                let range_start = request
                    .lines()
                    .find(|line| line.to_ascii_lowercase().starts_with("range:"))
                    .and_then(|line| line.split('=').nth(1))
                    .and_then(|value| value.trim().trim_end_matches('-').parse::<usize>().ok());

                match range_start {
                    Some(start) if start < payload.len() => {
                        let body = &payload[start..];
                        let header = format!(
                            "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {}-{}/{}\r\nConnection: close\r\n\r\n",
                            body.len(),
                            start,
                            payload.len() - 1,
                            payload.len()
                        );
                        let _ = stream.write_all(header.as_bytes());
                        let _ = stream.write_all(body);
                    }
                    _ => {
                        let header = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            payload.len()
                        );
                        let _ = stream.write_all(header.as_bytes());
                        let _ = stream.write_all(&payload);
                    }
                }
                let _ = stream.flush();
            }
            seen_requests
        });
        (format!("http://{addr}"), handle)
    }

    #[test]
    fn file_sha256_matches_reference() {
        let dir = temp_dir("sha");
        let file = dir.join("payload.bin");
        std::fs::write(&file, b"hello-world").expect("write");

        assert_eq!(
            file_sha256(&file).expect("hash"),
            sha256_of(b"hello-world")
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn verify_file_handles_missing_and_expected_empty() {
        let dir = temp_dir("verify");
        let file = dir.join("x.bin");
        assert!(!verify_file(&file, "abc").expect("verify missing"));
        assert!(!verify_file(&file, "").expect("verify empty expected"));

        std::fs::write(&file, b"data").expect("write");
        assert!(verify_file(&file, "").expect("exists"));
        assert!(verify_file(&file, &sha256_of(b"data")).expect("match"));
        assert!(!verify_file(&file, "deadbeef").expect("mismatch"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn download_writes_file_and_verifies_checksum() {
        let payload = b"model-binary-content".to_vec();
        let (base, handle) = spawn_file_server(payload.clone());
        let dir = temp_dir("download");
        let spec = spec_for(&payload);

        let mut progress_calls = 0u32;
        let mut on_progress = |downloaded: u64, _total: Option<u64>| {
            progress_calls += 1;
            assert!(downloaded <= payload.len() as u64);
        };

        let path = download_model(&dir, &spec, &base, &mut on_progress).expect("download");
        // 先释放闭包对 progress_calls 的可变借用，再断言
        drop(on_progress);

        assert!(path.exists());
        assert_eq!(std::fs::read(&path).expect("read"), payload);
        assert!(progress_calls > 0, "应上报进度");
        handle.join().ok();
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn download_resumes_from_partial_file_with_range() {
        let payload = b"0123456789abcdef".to_vec();
        let (base, handle) = spawn_file_server(payload.clone());
        let dir = temp_dir("resume");
        let spec = spec_for(&payload);

        // 预置前 6 个字节的半成品
        let target = model_file_path(&dir, &spec).expect("target");
        std::fs::create_dir_all(target.parent().expect("parent")).expect("mkdir");
        let part = part_file_path(&target);
        std::fs::write(&part, &payload[..6]).expect("partial");

        let mut on_progress = |_downloaded: u64, _total: Option<u64>| {};
        let path = download_model(&dir, &spec, &base, &mut on_progress).expect("resume");
        assert_eq!(std::fs::read(&path).expect("read"), payload);

        drop(on_progress);
        let requests = handle.join().unwrap_or_default();
        assert!(
            requests
                .iter()
                .any(|r| r.to_ascii_lowercase().contains("range: bytes=6-")),
            "续传时应携带 Range 头：{requests:?}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn download_rejects_checksum_mismatch_and_keeps_part_file() {
        let payload = b"tampered".to_vec();
        let (base, handle) = spawn_file_server(payload.clone());
        let dir = temp_dir("checksum");
        let mut spec = spec_for(&payload);
        spec.sha256 = "0".repeat(64);

        let mut on_progress = |_d: u64, _t: Option<u64>| {};
        let error = download_model(&dir, &spec, &base, &mut on_progress).expect_err("should fail");
        assert!(error.contains("SHA256"), "错误信息应说明校验失败：{error}");

        let target = model_file_path(&dir, &spec).expect("target");
        assert!(!target.exists(), "校验失败不应落盘正式文件");
        assert!(part_file_path(&target).exists(), ".part 应保留以便续传");

        drop(on_progress);
        handle.join().ok();
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn catalog_ids_are_unique_and_paths_are_safe() {
        let models = catalog();
        let mut ids: Vec<&String> = models.iter().map(|m| &m.id).collect();
        let total = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), total);

        for spec in &models {
            assert!(safe_relative_path(&spec.path).is_ok());
            assert!(safe_relative_path(&spec.file_name).is_ok());
        }
        assert!(safe_relative_path("../escape.bin").is_err());
        assert!(find_spec("asr-paraformer-zh").is_some());
        assert!(find_spec("unknown").is_none());
    }

    #[test]
    fn list_status_reports_installed_and_partial() {
        let dir = temp_dir("status");
        let spec = catalog().into_iter().next().expect("spec");
        let status = list_status(&dir);
        assert!(status.iter().all(|s| !s.installed));

        // 伪造一个已安装文件与一个半成品
        let installed = model_file_path(&dir, &spec).expect("path");
        std::fs::create_dir_all(installed.parent().expect("parent")).expect("mkdir");
        std::fs::write(&installed, b"x").expect("write");
        let asr = find_spec("asr-paraformer-zh").expect("asr");
        let asr_path = model_file_path(&dir, &asr).expect("asr path");
        std::fs::create_dir_all(asr_path.parent().expect("parent")).expect("mkdir");
        std::fs::write(part_file_path(&asr_path), b"12345").expect("part");

        let status = list_status(&dir);
        let installed_status = status.iter().find(|s| s.spec.id == spec.id).expect("entry");
        assert!(installed_status.installed);
        let asr_status = status.iter().find(|s| s.spec.id == asr.id).expect("entry");
        assert_eq!(asr_status.partial_bytes, 5);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn probe_url_reports_unconfigured_mirror() {
        let probe = probe_url("", "embedding/minilm-zh.onnx");
        assert!(!probe.ok);
        assert!(probe.message.contains("未配置"), "{}", probe.message);
        assert_eq!(probe.status, None);
    }

    #[test]
    fn probe_url_reports_unreachable_host() {
        // 127.0.0.1:1 通常无监听，应报告不可达而非 panic
        let probe = probe_url("http://127.0.0.1:1", "vad/silero_vad.onnx");
        assert!(!probe.ok);
        assert!(!probe.message.is_empty());
    }

    #[test]
    fn probe_url_detects_ok_and_range_support() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let handle = thread::spawn(move || {
            for stream in listener.incoming().take(1) {
                let Ok(mut stream) = stream else { continue };
                let mut buffer = [0u8; 1024];
                let _ = stream.read(&mut buffer);
                let header = "HTTP/1.1 200 OK\r\nContent-Length: 2048\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n";
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.flush();
            }
        });

        let probe = probe_url(&format!("http://{addr}"), "x/y.onnx");
        assert!(probe.ok, "应为可达：{}", probe.message);
        assert_eq!(probe.status, Some(200));
        assert_eq!(probe.content_length, Some(2048));
        assert!(probe.supports_range);

        handle.join().ok();
    }

    #[test]
    fn resolve_mirror_prefers_configured_over_env() {
        // 配置值优先于环境变量
        let resolved = resolve_mirror(Some("https://cfg.example.com/ai"));
        assert_eq!(resolved, "https://cfg.example.com/ai");

        // 空白配置视为未配置，回退环境变量
        let resolved = resolve_mirror(Some("   "));
        assert_eq!(resolved, std::env::var("AIA_MODEL_MIRROR").unwrap_or_default());
    }

    #[test]
    fn delete_model_removes_both_files() {
        let dir = temp_dir("delete");
        let spec = catalog().into_iter().next().expect("spec");
        let target = model_file_path(&dir, &spec).expect("path");
        std::fs::create_dir_all(target.parent().expect("parent")).expect("mkdir");
        std::fs::write(&target, b"data").expect("write");
        std::fs::write(part_file_path(&target), b"part").expect("write part");

        assert!(delete_model(&dir, &spec).expect("delete"));
        assert!(!target.exists());
        assert!(!part_file_path(&target).exists());
        assert!(!delete_model(&dir, &spec).expect("delete twice"));

        std::fs::remove_dir_all(&dir).ok();
    }
}
