//! Windows 本地 ASR 推理：sherpa-onnx（中文 paraformer）封装（IA-15）。
//!
//! 关键点：**特征提取（fbank / LFR / CMVN）与解码全部由 sherpa-onnx 原生库完成**，
//! 本模块只需把音频管线的输出（16kHz 单声道 f32 PCM）喂进去，因此与上层
//! [`LocalAsrEngine`](super::LocalAsrEngine) 的「段级识别」接口天然对齐。
//!
//! 原生库（`sherpa-onnx-c-api` + `onnxruntime`）由 `sherpa-rs-sys` 的 build.rs 按
//! target triple 下载预编译包并校验 SHA256；Windows 打包时需让 DLL 与 exe 同目录，
//! 见 `tauri.windows.conf.json` 的 `bundle.resources`。

use crate::providers::types::{ProviderError, ProviderResult};
use sherpa_rs::paraformer::{ParaformerConfig, ParaformerRecognizer};
use std::path::Path;

/// 解码线程数（paraformer int8 在 4 线程下已能满足 < 1s 的单段预算）
const NUM_THREADS: i32 = 4;

/// Paraformer 离线识别器封装
#[derive(Debug)]
pub struct ParaformerEngine {
    recognizer: ParaformerRecognizer,
}

impl ParaformerEngine {
    /// 加载模型（int8 约 200MB 内存，首次加载数百毫秒，故由上层按需懒加载）
    pub fn load(model: &Path, tokens: &Path) -> ProviderResult<Self> {
        let config = ParaformerConfig {
            model: model.to_string_lossy().to_string(),
            tokens: tokens.to_string_lossy().to_string(),
            provider: None, // 默认 CPU 执行器
            num_threads: Some(NUM_THREADS),
            debug: false,
        };

        let recognizer = ParaformerRecognizer::new(config).map_err(|err| {
            ProviderError::Other(format!(
                "加载本地 ASR 模型失败（{}）：{err}。模型文件可能损坏，请在「设置 → 模型管理」删除后重新下载。",
                model.display()
            ))
        })?;

        Ok(Self { recognizer })
    }

    /// 识别一段 16kHz 单声道 f32 PCM；无有效文本时返回 `None`
    pub fn transcribe(&mut self, samples: &[f32]) -> ProviderResult<Option<String>> {
        let result = self
            .recognizer
            .transcribe(crate::audio::TARGET_SAMPLE_RATE, samples);
        let text = result.text.trim();
        Ok(if text.is_empty() {
            None
        } else {
            Some(text.to_string())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 真实模型冒烟测试：需本机已下载 `asr-paraformer-zh`（默认忽略）。
    ///
    /// 手动执行：`cargo test -- --ignored asr::sherpa`
    /// 作用：验证原生库可加载、可解码（静音输入不应产出文本），
    /// 即「引擎能跑」而不是「能识别出正确内容」——后者需要真实语音样本。
    #[test]
    #[ignore = "需要本机已下载本地 ASR 模型（设置 → 模型管理）"]
    fn loads_real_model_and_decodes_silence() {
        let data_dir = std::env::var("APPDATA").unwrap_or_default();
        assert!(!data_dir.is_empty(), "Windows 需要 APPDATA 环境变量");
        let model_dir = std::path::PathBuf::from(data_dir)
            .join("com.easyinterview.app")
            .join("models")
            .join(crate::asr::LOCAL_ASR_MODEL_ID);
        let model = model_dir.join("model.int8.onnx");
        let tokens = model_dir.join("tokens.txt");
        if !model.exists() || !tokens.exists() {
            eprintln!("跳过：未下载模型（{}）", model_dir.display());
            return;
        }

        let mut engine = ParaformerEngine::load(&model, &tokens).expect("加载模型");
        // 1 秒静音：不应产出文本，但必须不报错
        let text = engine.transcribe(&vec![0.0; 16_000]).expect("识别静音");
        assert!(text.is_none(), "静音不应产出文本，实际：{text:?}");
    }
}
