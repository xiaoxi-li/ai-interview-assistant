//! 模型管理器：本地模型（嵌入 / VAD / 本地 ASR）的清单、下载与校验。
//!
//! 设计要点：
//! * 安装包不包含模型（保持 < 20MB），模型按需下载到应用数据目录；
//! * 下载源统一固定为魔搭社区（ModelScope），通过 HTTP 直链
//!   `https://modelscope.cn/models/{repo}/resolve/master/{path}` 下载；
//! * 每个模型可包含多个文件（如 sherpa-onnx 的 onnx + tokens，BGE 的 onnx + onnx_data）；
//! * 下载支持断点续传（HTTP Range），完成后做 SHA256 校验；
//! * 校验失败的半成品保留为 `.part` 文件，便于重新续传或手动清理。

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// 魔搭社区（ModelScope）默认下载源根地址
pub const MODELSCOPE_BASE: &str = "https://modelscope.cn";

/// 模型类别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelKind {
    Embedding,
    Vad,
    Asr,
}

/// 模型内的单个文件
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelFile {
    /// 模型仓库内的相对路径（用于拼接 `resolve/master/{remote_path}` 直链）
    pub remote_path: String,
    /// 本地落地文件名
    pub local_name: String,
    /// 期望的 SHA256（小写十六进制）；为空表示跳过校验
    pub sha256: String,
    /// 期望大小（字节，0 表示未知）
    pub size_bytes: u64,
}

/// 模型清单条目
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSpec {
    pub id: String,
    pub name: String,
    pub kind: ModelKind,
    /// ModelScope 仓库 ID（namespace/model）
    pub repo: String,
    /// 需要下载的文件列表
    pub files: Vec<ModelFile>,
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

/// 内置模型清单（下载源固定为魔搭社区）
pub fn catalog() -> Vec<ModelSpec> {
    vec![
        ModelSpec {
            id: "bge-m3".into(),
            name: "嵌入模型（BGE-M3）".into(),
            kind: ModelKind::Embedding,
            repo: "BAAI/bge-m3".into(),
            files: vec![
                ModelFile {
                    remote_path: "onnx/model.onnx".into(),
                    local_name: "model.onnx".into(),
                    sha256: String::new(),
                    size_bytes: 724_923,
                },
                ModelFile {
                    remote_path: "onnx/model.onnx_data".into(),
                    local_name: "model.onnx_data".into(),
                    sha256: String::new(),
                    size_bytes: 2_266_820_608,
                },
                ModelFile {
                    remote_path: "onnx/tokenizer.json".into(),
                    local_name: "tokenizer.json".into(),
                    sha256: String::new(),
                    size_bytes: 17_082_821,
                },
                ModelFile {
                    remote_path: "onnx/config.json".into(),
                    local_name: "config.json".into(),
                    sha256: String::new(),
                    size_bytes: 698,
                },
            ],
            description: "本地向量检索使用的多语言嵌入模型（1024 维），约 2.1GB".into(),
            required_for_local_asr: false,
        },
        ModelSpec {
            id: "vad-silero".into(),
            name: "Silero VAD".into(),
            kind: ModelKind::Vad,
            repo: "pengzhendong/silero-vad".into(),
            files: vec![ModelFile {
                remote_path: "silero_vad.onnx".into(),
                local_name: "silero_vad.onnx".into(),
                sha256: String::new(),
                size_bytes: 2_327_524,
            }],
            description: "语音活动检测模型，用于静音过滤与自动分段，约 2.3MB".into(),
            required_for_local_asr: false,
        },
        ModelSpec {
            id: "asr-paraformer-zh".into(),
            name: "本地 ASR（Paraformer 中文）".into(),
            kind: ModelKind::Asr,
            repo: "pengzhendong/sherpa-onnx-paraformer-zh".into(),
            files: vec![
                ModelFile {
                    remote_path: "model.int8.onnx".into(),
                    local_name: "model.int8.onnx".into(),
                    sha256: "90bc03034ae1bef9575f8cc798cd1519c8be8aa9e8b458a033e32017ff4d584c".into(),
                    size_bytes: 227_330_205,
                },
                ModelFile {
                    remote_path: "tokens.txt".into(),
                    local_name: "tokens.txt".into(),
                    sha256: "6c0e3b35cece259829e6cb5b8d90d13db88f61ea3a2953d11898e4b2bfd7a2e2".into(),
                    size_bytes: 75_354,
                },
                ModelFile {
                    remote_path: "configuration.json".into(),
                    local_name: "configuration.json".into(),
                    sha256: String::new(),
                    size_bytes: 56,
                },
            ],
            description: "本地语音识别模型，仅在选择本地 ASR 模式时需要，约 217MB".into(),
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

/// 模型在本地磁盘上的目录路径（多文件模型按 `models_dir/{model_id}/` 落地）
pub fn model_dir_path(models_dir: &Path, spec: &ModelSpec) -> Result<PathBuf, String> {
    Ok(models_dir.join(safe_relative_path(&spec.id)?))
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
            let dir = model_dir_path(models_dir, &spec).ok();
            let installed = dir
                .as_ref()
                .map(|d| {
                    spec.files
                        .iter()
                        .all(|file| d.join(&file.local_name).exists())
                })
                .unwrap_or(false);
            let partial_bytes = dir
                .as_ref()
                .map(|d| {
                    spec.files
                        .iter()
                        .map(|file| {
                            std::fs::metadata(part_file_path(&d.join(&file.local_name)))
                                .map(|m| m.len())
                                .unwrap_or(0)
                        })
                        .sum()
                })
                .unwrap_or(0);
            let resolved_path = dir.as_ref().map(|d| d.display().to_string());
            ModelStatus {
                resolved_path,
                installed,
                partial_bytes,
                spec,
            }
        })
        .collect()
}

/// 下载进度回调：(已下载字节, 总字节 Option)
pub type ProgressFn<'a> = &'a mut dyn FnMut(u64, Option<u64>);

/// 下载模型，支持多文件、断点续传与 SHA256 校验。
///
/// `base_url` 为下载源根地址（生产传 [`MODELSCOPE_BASE`]，测试可传本地服务地址）。
/// 成功后返回模型目录路径；校验失败会保留 `.part` 文件并返回错误。
pub fn download_model(
    models_dir: &Path,
    spec: &ModelSpec,
    base_url: &str,
    on_progress: &mut dyn FnMut(u64, Option<u64>),
) -> Result<PathBuf, String> {
    let model_dir = model_dir_path(models_dir, spec)?;
    std::fs::create_dir_all(&model_dir).map_err(|e| e.to_string())?;

    let total: u64 = spec.files.iter().map(|file| file.size_bytes).sum();
    let mut base_downloaded: u64 = 0;

    for file in &spec.files {
        let target = model_dir.join(&file.local_name);
        let part = part_file_path(&target);

        // 已完整安装且校验通过：跳过并计入进度
        if target.exists() && verify_file(&target, &file.sha256)? {
            base_downloaded += file.size_bytes;
            on_progress(base_downloaded, Some(total));
            continue;
        }

        // target 损坏或不存在：清理 target 避免 rename 冲突
        if target.exists() {
            std::fs::remove_file(&target).map_err(|e| e.to_string())?;
        }

        let url = format!(
            "{}/models/{}/resolve/master/{}",
            base_url.trim_end_matches('/'),
            spec.repo,
            file.remote_path
        );

        let base = base_downloaded;
        download_file(&url, &target, &part, &file.sha256, &mut |downloaded, _| {
            on_progress(base + downloaded, Some(total));
        })?;

        base_downloaded += file.size_bytes;
    }

    Ok(model_dir)
}

/// 下载单个文件，支持断点续传与 SHA256 校验（内部辅助）。
fn download_file(
    url: &str,
    target: &Path,
    part: &Path,
    sha256: &str,
    on_progress: &mut dyn FnMut(u64, Option<u64>),
) -> Result<(), String> {
    let mut offset = std::fs::metadata(part).map(|m| m.len()).unwrap_or(0);

    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(600))
        .build();
    let mut request = agent.get(url);
    if offset > 0 {
        request = request.set("Range", &format!("bytes={offset}-"));
    }

    let response = request.call().map_err(|e| format!("下载失败：{e}"))?;
    let status = response.status();

    // 服务端不支持续传（返回 200）时从头下载
    if offset > 0 && status != 206 {
        let _ = std::fs::remove_file(part);
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
        .open(part)
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
    if !sha256.trim().is_empty() {
        let actual = file_sha256(part)?;
        if !actual.eq_ignore_ascii_case(sha256) {
            return Err(format!(
                "SHA256 校验失败（期望 {sha256}，实际 {actual}），已保留 .part 文件可重新续传"
            ));
        }
    }

    std::fs::rename(part, target).map_err(|e| format!("落盘失败：{e}"))?;
    Ok(())
}

/// 删除已安装模型（清理整个模型目录）
pub fn delete_model(models_dir: &Path, spec: &ModelSpec) -> Result<bool, String> {
    let model_dir = model_dir_path(models_dir, spec)?;
    if !model_dir.exists() {
        return Ok(false);
    }
    std::fs::remove_dir_all(&model_dir).map_err(|e| e.to_string())?;
    Ok(true)
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
            repo: "test/test-model".into(),
            files: vec![ModelFile {
                remote_path: "test/model.bin".into(),
                local_name: "test-model.bin".into(),
                sha256: sha256_of(payload),
                size_bytes: payload.len() as u64,
            }],
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
    fn download_writes_files_and_verifies_checksum() {
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
        drop(on_progress);

        assert!(path.exists());
        assert_eq!(
            std::fs::read(path.join("test-model.bin")).expect("read"),
            payload
        );
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
        let model_dir = model_dir_path(&dir, &spec).expect("dir");
        std::fs::create_dir_all(&model_dir).expect("mkdir");
        let target = model_dir.join("test-model.bin");
        std::fs::write(part_file_path(&target), &payload[..6]).expect("partial");

        let mut on_progress = |_downloaded: u64, _total: Option<u64>| {};
        let path = download_model(&dir, &spec, &base, &mut on_progress).expect("resume");
        assert_eq!(
            std::fs::read(path.join("test-model.bin")).expect("read"),
            payload
        );

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
        spec.files[0].sha256 = "0".repeat(64);

        let mut on_progress = |_d: u64, _t: Option<u64>| {};
        let error = download_model(&dir, &spec, &base, &mut on_progress).expect_err("should fail");
        assert!(error.contains("SHA256"), "错误信息应说明校验失败：{error}");

        let model_dir = model_dir_path(&dir, &spec).expect("dir");
        let target = model_dir.join("test-model.bin");
        assert!(!target.exists(), "校验失败不应落盘正式文件");
        assert!(part_file_path(&target).exists(), ".part 应保留以便续传");

        drop(on_progress);
        handle.join().ok();
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn download_skips_installed_files_and_aggregates_progress() {
        let payload = b"second-file-content".to_vec();
        let (base, handle) = spawn_file_server(payload.clone());
        let dir = temp_dir("skip-installed");
        let mut spec = spec_for(&payload);
        spec.files.push(ModelFile {
            remote_path: "test/first.bin".into(),
            local_name: "first.bin".into(),
            sha256: sha256_of(b"first-file-content"),
            size_bytes: 18,
        });

        // 预置第一个文件已安装
        let model_dir = model_dir_path(&dir, &spec).expect("dir");
        std::fs::create_dir_all(&model_dir).expect("mkdir");
        std::fs::write(model_dir.join("first.bin"), b"first-file-content").expect("write");

        let mut progress = Vec::new();
        let mut on_progress = |downloaded: u64, total: Option<u64>| {
            progress.push((downloaded, total));
        };
        let path = download_model(&dir, &spec, &base, &mut on_progress).expect("download");
        assert_eq!(
            std::fs::read(path.join("test-model.bin")).expect("read"),
            payload
        );

        drop(on_progress);
        let requests = handle.join().unwrap_or_default();
        assert_eq!(requests.len(), 1, "应只请求未安装的文件：{requests:?}");

        let total = spec.files.iter().map(|f| f.size_bytes).sum::<u64>();
        assert_eq!(progress.last().map(|(_, t)| *t), Some(Some(total)));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn catalog_ids_are_unique_and_files_are_safe() {
        let models = catalog();
        let mut ids: Vec<&String> = models.iter().map(|m| &m.id).collect();
        let total = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), total);

        for spec in &models {
            assert!(!spec.repo.is_empty());
            assert!(!spec.files.is_empty(), "模型应有至少一个文件：{}", spec.id);
            for file in &spec.files {
                assert!(safe_relative_path(&file.local_name).is_ok());
                assert!(!file.remote_path.is_empty());
                assert!(!file.remote_path.contains(".."), "remote_path 不应包含目录穿越");
            }
        }
        assert!(safe_relative_path("../escape.bin").is_err());
        assert!(find_spec("asr-paraformer-zh").is_some());
        assert!(find_spec("bge-m3").is_some());
        assert!(find_spec("unknown").is_none());
    }

    #[test]
    fn list_status_reports_installed_and_partial() {
        let dir = temp_dir("status");
        let spec = catalog().into_iter().next().expect("spec");
        let status = list_status(&dir);
        assert!(status.iter().all(|s| !s.installed));

        // 伪造一个已安装模型（所有文件存在）
        let model_dir = model_dir_path(&dir, &spec).expect("dir");
        std::fs::create_dir_all(&model_dir).expect("mkdir");
        for file in &spec.files {
            std::fs::write(model_dir.join(&file.local_name), b"x").expect("write");
        }

        // 伪造 ASR 模型的一个半成品
        let asr = find_spec("asr-paraformer-zh").expect("asr");
        let asr_dir = model_dir_path(&dir, &asr).expect("asr dir");
        std::fs::create_dir_all(&asr_dir).expect("mkdir");
        std::fs::write(part_file_path(&asr_dir.join("model.int8.onnx")), b"12345").expect("part");

        let status = list_status(&dir);
        let installed_status = status.iter().find(|s| s.spec.id == spec.id).expect("entry");
        assert!(installed_status.installed);
        let asr_status = status.iter().find(|s| s.spec.id == asr.id).expect("entry");
        assert_eq!(asr_status.partial_bytes, 5);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn delete_model_removes_model_dir() {
        let dir = temp_dir("delete");
        let spec = catalog().into_iter().next().expect("spec");
        let model_dir = model_dir_path(&dir, &spec).expect("dir");
        std::fs::create_dir_all(&model_dir).expect("mkdir");
        for file in &spec.files {
            std::fs::write(model_dir.join(&file.local_name), b"data").expect("write");
        }

        assert!(delete_model(&dir, &spec).expect("delete"));
        assert!(!model_dir.exists());
        assert!(!delete_model(&dir, &spec).expect("delete twice"));

        std::fs::remove_dir_all(&dir).ok();
    }
}
