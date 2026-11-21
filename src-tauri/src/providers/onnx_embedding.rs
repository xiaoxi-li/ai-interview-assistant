//! 本地 ONNX 嵌入推理（TODO-07 / KB-07）：BAAI/bge-m3（1024 维）。
//!
//! 设计要点与 `asr::sherpa`（本地 ASR）保持一致：
//! * **懒加载**：`AppState.embedding` 是 `Arc<dyn EmbeddingProvider>`，进程内只在首次
//!   `embed()` 时加载 Session 与 tokenizer（模型约 2.2GB，加载耗时数百 ms ~ 数秒）；
//! * **未就绪即失败**：模型未下载 / 特性未启用时返回可操作错误，由上层检索链路自动
//!   **降级为纯关键词检索**（`retrieval::search_hybrid` 既有行为），不阻塞笔记写入；
//! * **池化与归一化在本模块完成**：优先取官方导出的 `sentence_embedding`；若只有
//!   `last_hidden_state`（[batch, seq, hidden]），取 `[CLS]`（第 0 个 token）向量。
//!   随后做 L2 归一化——让 sqlite-vec 的 L2 距离等价于余弦相似度；
//! * **query 侧指令前缀**：bge 系列建议对检索 query 拼接任务指令，文档侧不拼。

use crate::providers::types::{ProviderError, ProviderResult};
use crate::providers::EmbeddingProvider;
use crate::storage::EMBEDDING_DIM;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// bge-m3 在模型清单中的 id（`models::catalog()`）
pub use crate::providers::LOCAL_EMBEDDING_MODEL_ID as BGE_M3_MODEL_ID;

/// bge 系列检索 query 的推荐指令前缀（文档侧不拼）
pub const BGE_QUERY_INSTRUCTION: &str = "为这个句子生成表示以用于检索相关文章：";

/// 单条文本的最大 token 数（超出截断）
const MAX_SEQ_LEN: usize = 512;

/// 模型输入/输出张量名（不同导出可能不同，按实际模型图探测）
struct TensorNames {
    input_ids: String,
    attention_mask: Option<String>,
    token_type_ids: Option<String>,
    /// 优先使用的输出名（`sentence_embedding` 优先，否则 `last_hidden_state`）
    output: String,
}

/// 已加载的推理资源（Session 非 Sync，故整体放入 Mutex）
struct Loaded {
    /// ONNX Runtime 环境必须与 Session 同生命周期
    _environment: ort::environment::Environment,
    session: ort::session::Session,
    tokenizer: tokenizers::Tokenizer,
    names: TensorNames,
}

/// 本地 ONNX 嵌入提供者（懒加载，load-dynamic 运行期绑定 ONNX Runtime）
pub struct OnnxEmbeddingProvider {
    model_dir: PathBuf,
    dim: usize,
    inner: Mutex<Option<Loaded>>,
}

impl OnnxEmbeddingProvider {
    /// 从模型目录构造（不加载模型，只做文件就绪校验）
    ///
    /// 模型缺失返回可操作错误，调用方据此决定是否回退 Mock 嵌入。
    pub fn from_models_dir(models_dir: &Path) -> ProviderResult<Self> {
        let spec = crate::models::catalog()
            .into_iter()
            .find(|spec| spec.id == BGE_M3_MODEL_ID)
            .ok_or_else(|| ProviderError::Other("模型清单缺少 bge-m3 条目".to_string()))?;
        let model_dir =
            crate::models::model_dir_path(models_dir, &spec).map_err(ProviderError::Other)?;
        let model_path = model_dir.join("model.onnx");
        let tokenizer_path = model_dir.join("tokenizer.json");
        if !model_path.exists() || !tokenizer_path.exists() {
            return Err(ProviderError::Other(format!(
                "本地嵌入模型尚未下载（缺少 {}）：请前往「设置 → 模型管理」下载「{}」后重试。",
                model_path.display(),
                spec.name
            )));
        }
        Ok(Self {
            model_dir,
            dim: EMBEDDING_DIM,
            inner: Mutex::new(None),
        })
    }

    /// 模型目录（供状态展示与日志）
    pub fn model_dir(&self) -> &Path {
        &self.model_dir
    }

    /// 是否已完成一次加载（就绪状态探测用，不做真实推理）
    pub fn is_loaded(&self) -> bool {
        self.inner
            .lock()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
    }

    /// 确保 Session 与 tokenizer 已加载（首次调用会有明显耗时）
    fn ensure_loaded(&self) -> ProviderResult<std::sync::MutexGuard<'_, Option<Loaded>>> {
        let mut guard = self.inner.lock().map_err(|_| {
            ProviderError::Other("嵌入引擎锁中毒，请重启应用".to_string())
        })?;
        if guard.is_none() {
            *guard = Some(load_resources(&self.model_dir)?);
        }
        Ok(guard)
    }
}

/// 加载 Session + tokenizer（只在首次调用时执行）
fn load_resources(model_dir: &Path) -> ProviderResult<Loaded> {
    let model_path = model_dir.join("model.onnx");
    let tokenizer_path = model_dir.join("tokenizer.json");

    // ONNX Runtime 环境（load-dynamic：运行期加载 onnxruntime.dll，
    // 开发构建由 ort 的 download-binaries 复制到 target 目录）
    let environment = ort::init()
        .with_name("easy-interview-embedding")
        .build()
        .map_err(|err| ProviderError::Other(format!("初始化 ONNX Runtime 环境失败：{err}")))?;

    let mut builder = ort::session::Session::builder(&environment)
        .map_err(|err| ProviderError::Other(format!("创建 ONNX 会话构建器失败：{err}")))?;
    builder = builder
        .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
        .map_err(|err| ProviderError::Other(format!("设置图优化级别失败：{err}")))?;
    builder = builder
        .with_intra_threads(2)
        .map_err(|err| ProviderError::Other(format!("设置推理线程数失败：{err}")))?;
    let session = builder.commit_from_file(&model_path).map_err(|err| {
        ProviderError::Other(format!(
            "加载本地嵌入模型失败（{}）：{err}。请确认模型文件完整，或在「设置 → 模型管理」重新下载。",
            model_path.display()
        ))
    })?;

    let tokenizer = tokenizers::Tokenizer::from_file(&tokenizer_path).map_err(|err| {
        ProviderError::Other(format!(
            "加载分词器失败（{}）：{err}。请重新下载「{}」。",
            tokenizer_path.display(),
            BGE_M3_MODEL_ID
        ))
    })?;

    let names = detect_tensor_names(&session);
    Ok(Loaded {
        _environment: environment,
        session,
        tokenizer,
        names,
    })
}

/// 探测输入/输出名：bge-m3 官方导出为 `input_ids` / `attention_mask` / `token_type_ids`，
/// 输出为 `last_hidden_state`（部分导出额外提供 `sentence_embedding`）。
fn detect_tensor_names(session: &ort::session::Session) -> TensorNames {
    let inputs: Vec<String> = session
        .inputs()
        .iter()
        .map(|input| input.name().to_string())
        .collect();
    let outputs: Vec<String> = session
        .outputs()
        .iter()
        .map(|output| output.name().to_string())
        .collect();
    let pick_input = |candidate: &str| -> Option<String> {
        inputs
            .iter()
            .find(|name| name.as_str() == candidate)
            .cloned()
    };
    let pick_output = |candidate: &str| -> Option<String> {
        outputs
            .iter()
            .find(|name| name.as_str() == candidate)
            .cloned()
    };
    TensorNames {
        input_ids: pick_input("input_ids")
            .or_else(|| inputs.first().cloned())
            .unwrap_or_else(|| "input_ids".to_string()),
        attention_mask: pick_input("attention_mask"),
        token_type_ids: pick_input("token_type_ids"),
        output: pick_output("sentence_embedding")
            .or_else(|| pick_output("last_hidden_state"))
            .or_else(|| outputs.first().cloned())
            .unwrap_or_else(|| "last_hidden_state".to_string()),
    }
}

impl EmbeddingProvider for OnnxEmbeddingProvider {
    fn name(&self) -> &str {
        "local-onnx"
    }

    fn dim(&self) -> usize {
        self.dim
    }

    fn embed(&self, texts: &[String]) -> ProviderResult<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let mut guard = self.ensure_loaded()?;
        let loaded = guard
            .as_mut()
            .ok_or_else(|| ProviderError::Other("嵌入引擎未初始化".to_string()))?;
        // 逐条推理：单条 512 token 的延迟在百毫秒级，批量拼图会拉高内存峰值
        let mut vectors = Vec::with_capacity(texts.len());
        for text in texts {
            vectors.push(embed_one(loaded, text)?);
        }
        Ok(vectors)
    }
}

/// 单条推理：分词 → 组输入张量 → 执行 → 取句向量 → L2 归一化
fn embed_one(loaded: &mut Loaded, text: &str) -> ProviderResult<Vec<f32>> {
    let encoding = loaded
        .tokenizer
        .encode(text, true)
        .map_err(|err| ProviderError::Other(format!("分词失败：{err}")))?;

    let ids: Vec<i64> = encoding
        .get_ids()
        .iter()
        .take(MAX_SEQ_LEN)
        .map(|&id| i64::from(id))
        .collect();
    let mask: Vec<i64> = encoding
        .get_attention_mask()
        .iter()
        .take(MAX_SEQ_LEN)
        .map(|&value| i64::from(value))
        .collect();
    if ids.is_empty() {
        return Ok(vec![0.0f32; EMBEDDING_DIM]);
    }
    let seq_len = ids.len();

    run_inference(loaded, ids, mask, seq_len)
}

/// 组装输入并执行推理，返回归一化后的句向量
fn run_inference(
    loaded: &mut Loaded,
    ids: Vec<i64>,
    mask: Vec<i64>,
    seq_len: usize,
) -> ProviderResult<Vec<f32>> {
    let shape = vec![1_i64, seq_len as i64];
    let ids_tensor = ort::value::Tensor::from_array((shape.clone(), ids)).map_err(tensor_error)?;
    let mask_tensor =
        ort::value::Tensor::from_array((shape.clone(), mask.clone())).map_err(tensor_error)?;

    let names = &loaded.names;
    let mut feeds: Vec<(String, ort::value::Tensor<i64>)> =
        vec![(names.input_ids.clone(), ids_tensor)];
    if let Some(name) = &names.attention_mask {
        feeds.push((name.clone(), mask_tensor));
    }
    if let Some(name) = &names.token_type_ids {
        let zeros: Vec<i64> = vec![0; seq_len];
        let tensor = ort::value::Tensor::from_array((shape, zeros)).map_err(tensor_error)?;
        feeds.push((name.clone(), tensor));
    }

    let inputs: Vec<(
        std::borrow::Cow<'_, str>,
        ort::session::SessionInputValue<'_>,
    )> = feeds
        .iter()
        .map(|(name, tensor)| (name.as_str().into(), tensor.into()))
        .collect();

    let outputs = loaded
        .session
        .run(ort::session::SessionInputs::from(inputs))
        .map_err(|err| ProviderError::Other(format!("嵌入推理失败：{err}")))?;

    extract_sentence_vector(&outputs, &names.output)
}

/// 从输出中取句向量并归一化
fn extract_sentence_vector(
    outputs: &ort::session::SessionOutputs<'_>,
    preferred: &str,
) -> ProviderResult<Vec<f32>> {
    // 优先取探测到的输出名；模型图若命名不同则退到第一个输出
    let name = if outputs.get(preferred).is_some() {
        preferred.to_string()
    } else {
        outputs
            .iter()
            .next()
            .map(|(name, _)| name.to_string())
            .ok_or_else(|| ProviderError::Other("嵌入模型没有输出".to_string()))?
    };
    let value = outputs
        .get(&name)
        .ok_or_else(|| ProviderError::Other("读取嵌入输出失败".to_string()))?;
    let (shape, data) = value
        .try_extract_tensor::<f32>()
        .map_err(|err| ProviderError::Other(format!("读取嵌入输出失败：{err}")))?;
    let dims: &[i64] = shape.as_ref();
    let hidden = EMBEDDING_DIM;

    let start = match dims.len() {
        // [batch, seq, hidden]：取 [CLS]（第 0 个 token）
        3 => 0,
        // [batch, hidden]：直接就是句向量
        2 => 0,
        other => {
            return Err(ProviderError::Other(format!(
                "嵌入输出维度异常（期望 2 或 3 维，实际 {other} 维）"
            )))
        }
    };
    if data.len() < start + hidden {
        return Err(ProviderError::Other(format!(
            "嵌入输出长度不足（期望至少 {} 个元素，实际 {}）",
            start + hidden,
            data.len()
        )));
    }
    let mut vector = data[start..start + hidden].to_vec();
    l2_normalize(&mut vector);
    Ok(vector)
}

fn l2_normalize(vector: &mut [f32]) {
    let norm = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in vector.iter_mut() {
            *value /= norm;
        }
    }
}

fn tensor_error(err: impl std::fmt::Display) -> ProviderError {
    ProviderError::Other(format!("构造嵌入输入张量失败：{err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_models_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "easy-interview-embed-test-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("创建临时模型目录");
        dir
    }

    #[test]
    fn missing_model_reports_actionable_error() {
        let dir = temp_models_dir("missing");
        let err = OnnxEmbeddingProvider::from_models_dir(&dir)
            .err()
            .expect("模型缺失应返回错误");
        let message = err.to_string();
        assert!(message.contains("尚未下载"), "{message}");
        assert!(message.contains("模型管理"), "应给出可操作引导：{message}");
        assert!(message.contains("BGE-M3") || message.contains("bge"), "{message}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn partially_downloaded_model_still_reported_missing() {
        let dir = temp_models_dir("partial");
        // 只有 model.onnx，缺 tokenizer.json
        let model_dir = dir.join(BGE_M3_MODEL_ID);
        std::fs::create_dir_all(&model_dir).expect("创建模型目录");
        std::fs::write(model_dir.join("model.onnx"), b"not a real model").expect("写占位文件");

        let err = OnnxEmbeddingProvider::from_models_dir(&dir)
            .err()
            .expect("缺 tokenizer 应返回错误");
        assert!(err.to_string().contains("尚未下载"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn l2_normalize_produces_unit_vector() {
        let mut vector = vec![3.0f32, 4.0];
        l2_normalize(&mut vector);
        let norm = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-6, "归一化后长度应为 1，实际 {norm}");
        assert!((vector[0] - 0.6).abs() < 1e-6);
        assert!((vector[1] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn l2_normalize_keeps_zero_vector() {
        let mut vector = vec![0.0f32; 8];
        l2_normalize(&mut vector);
        assert!(vector.iter().all(|value| *value == 0.0));
    }

    #[test]
    fn query_instruction_is_non_empty_chinese_prompt() {
        assert!(!BGE_QUERY_INSTRUCTION.is_empty());
        assert!(BGE_QUERY_INSTRUCTION.contains("检索"), "指令应说明用途");
    }

    /// 真实模型冒烟（需先在「设置 → 模型管理」下载 BGE-M3，并以 --features local-embed-native 编译）：
    /// `cargo test --features local-embed-native -- --ignored embedding_real`
    #[test]
    #[ignore = "需要本机已下载 BGE-M3 模型（设置 → 模型管理）"]
    fn embedding_real_model_produces_unit_vectors() {
        let data_dir = std::env::var("APPDATA").unwrap_or_default();
        let models_dir = PathBuf::from(data_dir)
            .join("com.easyinterview.app")
            .join("models");
        let provider = OnnxEmbeddingProvider::from_models_dir(&models_dir)
            .expect("BGE-M3 模型应已就绪");

        let texts = vec![
            "缓存击穿是指热点 key 失效瞬间大量请求打到数据库".to_string(),
            "Redis 缓存雪崩与击穿的区别".to_string(),
        ];
        let vectors = provider.embed(&texts).expect("推理应成功");
        assert_eq!(vectors.len(), 2);
        for vector in &vectors {
            assert_eq!(vector.len(), EMBEDDING_DIM);
            let norm = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!((norm - 1.0).abs() < 1e-3, "向量应为单位长度，实际 {norm}");
        }
        // 语义相关文本的余弦相似度应明显高于无关文本
        let unrelated = provider
            .embed(&["今天天气不错适合出门散步".to_string()])
            .expect("推理应成功");
        let related_cos = dot(&vectors[0], &vectors[1]);
        let unrelated_cos = dot(&vectors[0], &unrelated[0]);
        assert!(
            related_cos > unrelated_cos + 0.05,
            "相关文本相似度应更高：related={related_cos}, unrelated={unrelated_cos}"
        );

        fn dot(a: &[f32], b: &[f32]) -> f32 {
            a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
        }
    }
}
