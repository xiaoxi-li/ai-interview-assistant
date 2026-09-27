//! 本地 ASR 引擎抽象与实现。
//!
//! P0 决策：ASR 只走本地通路。引擎以「段级识别」为接口——
//! VAD 负责切段，段内可反复调用以产出 partial，段结束调用一次产出 final。
//!
//! 可用的引擎实现：
//! * [`SherpaOnnxEngine`]：生产引擎（sherpa-onnx 中文 paraformer）。模型由 ST-10
//!   模型管理器下载；**Windows 已接入 native 推理**（IA-15），识别器按需懒加载、
//!   `finish()` 释放。
//!
//!   ⚠️ 该通路由 cargo 特性 `local-asr-native` 控制（默认关闭）：启用后构建机需安装
//!   `libclang`（bindgen 依赖，见 README「本地 ASR 原生推理的构建前置」）。
//!   未启用时保持显式报错，不静默降级。macOS 接入见需求分析文档的 IA-15 待办。
//! * [`ReplayEngine`]：确定性回放引擎，仅用于单元/集成测试与「演示模式」，
//!   绝不用于生产识别。

#[cfg(all(windows, feature = "local-asr-native"))]
use crate::asr::sherpa::ParaformerEngine;
use crate::models::{find_spec, model_dir_path};
use crate::providers::types::{ProviderError, ProviderResult};
use std::path::{Path, PathBuf};

/// 本地 ASR 模型在模型清单中的 ID
pub const LOCAL_ASR_MODEL_ID: &str = "asr-paraformer-zh";

/// 段级本地识别引擎
pub trait LocalAsrEngine: Send {
    /// 引擎标识（用于界面与日志）
    fn name(&self) -> &str;

    /// 识别一段 16kHz 单声道 PCM（f32）。
    ///
    /// * `is_final = false`：段内中间结果（partial），可被后续调用覆盖；
    /// * `is_final = true`：该段已结束，结果是最终文本。
    ///
    /// 返回 `Ok(None)` 表示该段无有效语音文本。
    fn transcribe_segment(
        &mut self,
        samples: &[f32],
        is_final: bool,
    ) -> ProviderResult<Option<String>>;

    /// 结束会话（冲刷缓存、释放资源）
    fn finish(&mut self) -> ProviderResult<Option<String>>;
}

/// sherpa-onnx 本地引擎
#[derive(Debug)]
pub struct SherpaOnnxEngine {
    model_path: PathBuf,
    language: String,
    /// Windows + `local-asr-native`：懒加载的 paraformer 识别器（首次识别才加载模型，`finish()` 释放）
    #[cfg(all(windows, feature = "local-asr-native"))]
    recognizer: Option<ParaformerEngine>,
}

impl SherpaOnnxEngine {
    /// 构造引擎并校验模型是否就绪；缺失时返回**可操作**的错误提示。
    pub fn new(models_dir: &Path, language: &str) -> ProviderResult<Self> {
        let spec = find_spec(LOCAL_ASR_MODEL_ID)
            .ok_or_else(|| ProviderError::Other("模型清单中缺少本地 ASR 模型定义".into()))?;
        let model_path = model_dir_path(models_dir, &spec).map_err(ProviderError::Other)?;

        let onnx = model_path.join("model.int8.onnx");
        let tokens = model_path.join("tokens.txt");
        if !onnx.exists() || !tokens.exists() {
            return Err(ProviderError::Other(format!(
                "本地 ASR 模型尚未下载（缺少 model.int8.onnx 或 tokens.txt）：{}。请前往「设置 → 模型管理」下载「{}」后重试。",
                model_path.display(),
                spec.name
            )));
        }

        Ok(Self {
            model_path,
            language: language.to_string(),
            #[cfg(all(windows, feature = "local-asr-native"))]
            recognizer: None,
        })
    }

    pub fn model_path(&self) -> &Path {
        &self.model_path
    }

    pub fn language(&self) -> &str {
        &self.language
    }

    /// 懒加载识别器：避免仅仅查询状态就占用数百 MB 内存
    #[cfg(all(windows, feature = "local-asr-native"))]
    fn ensure_recognizer(&mut self) -> ProviderResult<&mut ParaformerEngine> {
        if self.recognizer.is_none() {
            let onnx = self.model_path.join("model.int8.onnx");
            let tokens = self.model_path.join("tokens.txt");
            self.recognizer = Some(ParaformerEngine::load(&onnx, &tokens)?);
        }
        self.recognizer
            .as_mut()
            .ok_or_else(|| ProviderError::Other("本地 ASR 识别器初始化失败".into()))
    }
}

impl LocalAsrEngine for SherpaOnnxEngine {
    fn name(&self) -> &str {
        "sherpa-onnx"
    }

    #[cfg(all(windows, feature = "local-asr-native"))]
    fn transcribe_segment(
        &mut self,
        samples: &[f32],
        _is_final: bool,
    ) -> ProviderResult<Option<String>> {
        if samples.is_empty() {
            return Ok(None);
        }
        // paraformer 为非流式模型：partial 由上层对累积音频反复整体解码得到
        self.ensure_recognizer()?.transcribe(samples)
    }

    #[cfg(not(all(windows, feature = "local-asr-native")))]
    fn transcribe_segment(
        &mut self,
        samples: &[f32],
        _is_final: bool,
    ) -> ProviderResult<Option<String>> {
        if samples.is_empty() {
            return Ok(None);
        }
        // 未启用原生推理：显式报错而非静默降级（宁可不产出文本，也不伪造）
        #[cfg(windows)]
        let reason = "当前构建未启用本地 ASR 原生推理：请在安装 libclang 的构建机上用 `--features local-asr-native` 重新构建（见 README）";
        #[cfg(not(windows))]
        let reason =
            "当前平台的本地 ASR 推理待接入：Windows 已支持（IA-15），macOS 计划见 ai-doc/需求分析.md 的 IA-15 待办";

        Err(ProviderError::Other(format!(
            "{reason}（模型已就绪：{}）。",
            self.model_path.display()
        )))
    }

    fn finish(&mut self) -> ProviderResult<Option<String>> {
        // 释放识别器，回收模型占用（int8 约 200MB）
        #[cfg(all(windows, feature = "local-asr-native"))]
        {
            self.recognizer = None;
        }
        Ok(None)
    }
}

/// 确定性回放引擎：仅用于测试与显式「演示模式」。
///
/// 行为：按 final 次数依次吐出脚本中的文本；未提供脚本时按时长生成占位文本。
/// partial 与 final 返回相同文本（不做增量推断），便于测试断言 partial→final 序列。
pub struct ReplayEngine {
    script: Vec<String>,
    cursor: usize,
}

impl Default for ReplayEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplayEngine {
    pub fn new() -> Self {
        Self {
            script: Vec::new(),
            cursor: 0,
        }
    }

    /// 使用固定脚本构造（按段序号依次取用）
    pub fn with_script(script: Vec<String>) -> Self {
        Self { script, cursor: 0 }
    }

    /// 已完成的段数
    pub fn emitted_segments(&self) -> usize {
        self.cursor
    }

    fn text_for(&self, samples: &[f32]) -> String {
        if let Some(text) = self.script.get(self.cursor) {
            return text.clone();
        }
        let duration_ms = (samples.len() as u64 * 1000) / crate::audio::TARGET_SAMPLE_RATE as u64;
        format!("[回放] 音频片段 {}（{duration_ms}ms）", self.cursor + 1)
    }
}

impl LocalAsrEngine for ReplayEngine {
    fn name(&self) -> &str {
        "replay"
    }

    fn transcribe_segment(
        &mut self,
        samples: &[f32],
        is_final: bool,
    ) -> ProviderResult<Option<String>> {
        if samples.is_empty() {
            return Ok(None);
        }
        let text = self.text_for(samples);
        if is_final {
            self.cursor += 1;
        }
        Ok(Some(text))
    }

    fn finish(&mut self) -> ProviderResult<Option<String>> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Db;
    use std::path::PathBuf;

    fn temp_dir(tag: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "aia-asr-{tag}-{}-{}",
            std::process::id(),
            crate::knowledge::store::now_ms()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn sherpa_engine_reports_missing_model_with_actionable_message() {
        let dir = temp_dir("missing");
        let error = SherpaOnnxEngine::new(&dir, "zh-CN").expect_err("应因缺少模型失败");
        let message = error.to_string();

        assert!(message.contains("模型管理"), "应引导用户去下载模型：{message}");
        assert!(message.contains("asr-paraformer-zh") || message.contains("本地 ASR"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn sherpa_engine_loads_when_model_file_exists() {
        let dir = temp_dir("present");
        let db = Db::open_in_memory().expect("db");
        let _ = db; // 仅确保存储层可初始化（模型路径不依赖 DB）

        // 伪造模型文件（onnx + tokens）
        let spec = find_spec(LOCAL_ASR_MODEL_ID).expect("spec");
        let model_dir = model_dir_path(&dir, &spec).expect("path");
        std::fs::create_dir_all(&model_dir).expect("mkdir");
        std::fs::write(model_dir.join("model.int8.onnx"), b"fake-model").expect("write");
        std::fs::write(model_dir.join("tokens.txt"), b"tokens").expect("write");

        let engine = SherpaOnnxEngine::new(&dir, "zh-CN").expect("engine");
        assert_eq!(engine.name(), "sherpa-onnx");
        assert_eq!(engine.language(), "zh-CN");
        assert!(engine.model_path().exists());

        // 伪造的模型文件无法被真正加载：应给出可操作的错误，而不是返回空文本
        let mut engine = engine;
        let error = engine
            .transcribe_segment(&vec![0.1; 320], true)
            .expect_err("伪造模型应报错");
        let message = error.to_string();
        // 启用原生推理时：伪造模型应报「加载失败」并引导重下；否则应报「未启用/待接入」
        #[cfg(all(windows, feature = "local-asr-native"))]
        assert!(
            message.contains("加载本地 ASR 模型失败") && message.contains("重新下载"),
            "启用原生推理时应报模型加载失败并引导重下：{message}"
        );
        #[cfg(not(all(windows, feature = "local-asr-native")))]
        assert!(
            message.contains("未启用") || message.contains("待接入"),
            "{message}"
        );

        // finish 应可重复调用（幂等），且不产出文本
        assert!(engine.finish().expect("finish").is_none());
        assert!(engine.finish().expect("finish again").is_none());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn sherpa_engine_returns_none_for_empty_audio() {
        let dir = temp_dir("empty");
        let spec = find_spec(LOCAL_ASR_MODEL_ID).expect("spec");
        let model_dir = model_dir_path(&dir, &spec).expect("path");
        std::fs::create_dir_all(&model_dir).expect("mkdir");
        std::fs::write(model_dir.join("model.int8.onnx"), b"x").expect("write");
        std::fs::write(model_dir.join("tokens.txt"), b"x").expect("write");

        let mut engine = SherpaOnnxEngine::new(&dir, "zh-CN").expect("engine");
        assert!(engine.transcribe_segment(&[], true).expect("ok").is_none());
        assert!(engine.finish().expect("finish").is_none());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn replay_engine_follows_script_and_advances_on_final() {
        let mut engine = ReplayEngine::with_script(vec!["第一段".into(), "第二段".into()]);
        let samples = vec![0.5f32; 320];

        // partial 不推进游标
        for _ in 0..3 {
            let text = engine.transcribe_segment(&samples, false).expect("partial");
            assert_eq!(text.as_deref(), Some("第一段"));
        }
        assert_eq!(engine.emitted_segments(), 0);

        assert_eq!(
            engine.transcribe_segment(&samples, true).expect("final").as_deref(),
            Some("第一段")
        );
        assert_eq!(engine.emitted_segments(), 1);
        assert_eq!(
            engine.transcribe_segment(&samples, true).expect("final").as_deref(),
            Some("第二段")
        );
        assert_eq!(engine.emitted_segments(), 2);
    }

    #[test]
    fn replay_engine_falls_back_to_duration_placeholder() {
        let mut engine = ReplayEngine::new();
        // 320 样本 @16kHz = 20ms
        let text = engine
            .transcribe_segment(&vec![0.5; 320], true)
            .expect("text")
            .expect("some");
        assert!(text.contains("20ms"), "实际：{text}");
        assert!(text.contains("音频片段 1"));
    }

    #[test]
    fn replay_engine_returns_none_for_empty_audio() {
        let mut engine = ReplayEngine::new();
        assert!(engine.transcribe_segment(&[], false).expect("ok").is_none());
        assert!(engine.transcribe_segment(&[], true).expect("ok").is_none());
    }
}
