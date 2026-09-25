//! Embedding Provider：本地 ONNX 小模型 / 云端 API 统一抽象。

use super::types::ProviderResult;
use crate::storage::EMBEDDING_DIM;

/// 文本嵌入抽象
pub trait EmbeddingProvider: Send + Sync {
    /// Provider 标识（如 local-onnx / cloud / mock）
    fn name(&self) -> &str;

    /// 向量维度
    fn dim(&self) -> usize;

    /// 批量嵌入
    fn embed(&self, texts: &[String]) -> ProviderResult<Vec<Vec<f32>>>;
}

/// 离线开发与测试用的 Mock Embedding：
/// 基于字节哈希生成确定性向量并做 L2 归一化，相同文本必得相同向量。
pub struct MockEmbeddingProvider {
    dim: usize,
}

impl MockEmbeddingProvider {
    pub fn new(dim: usize) -> Self {
        Self { dim }
    }
}

impl Default for MockEmbeddingProvider {
    fn default() -> Self {
        Self::new(EMBEDDING_DIM)
    }
}

fn pseudo_embed(text: &str, dim: usize) -> Vec<f32> {
    let mut v = vec![0.0f32; dim];
    if text.is_empty() || dim == 0 {
        return v;
    }
    for (i, byte) in text.bytes().enumerate() {
        v[i % dim] += f32::from(byte) / 255.0;
    }
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in &mut v {
            *x /= norm;
        }
    }
    v
}

impl EmbeddingProvider for MockEmbeddingProvider {
    fn name(&self) -> &str {
        "mock"
    }

    fn dim(&self) -> usize {
        self.dim
    }

    fn embed(&self, texts: &[String]) -> ProviderResult<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|t| pseudo_embed(t, self.dim)).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embed_is_deterministic() {
        let p = MockEmbeddingProvider::default();
        let a = p.embed(&["面试 问题".to_string()]).expect("embed");
        let b = p.embed(&["面试 问题".to_string()]).expect("embed");
        assert_eq!(a, b);
    }

    #[test]
    fn embed_dimension_matches_provider() {
        let p = MockEmbeddingProvider::new(64);
        let out = p.embed(&["hello".to_string(), "world".to_string()]).expect("embed");
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|v| v.len() == 64));
        assert_eq!(p.dim(), 64);
    }

    #[test]
    fn embed_is_l2_normalized() {
        let p = MockEmbeddingProvider::default();
        let v = p.embed(&["归一化".to_string()]).expect("embed").remove(0);
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5, "norm = {norm}");
    }

    #[test]
    fn different_text_yields_different_vector() {
        // 注意：Mock 向量在归一化后只保留"方向"信息，
        // 因此用例需选择方向确实不同的文本（如字符分布不同），
        // 而非 "aaa" / "bbb" 这类同向仅长度不同的字符串。
        let p = MockEmbeddingProvider::default();
        let a = p.embed(&["abc".to_string()]).expect("embed");
        let b = p.embed(&["xyz".to_string()]).expect("embed");
        assert_ne!(a, b);
    }

    #[test]
    fn empty_text_yields_zero_vector() {
        let p = MockEmbeddingProvider::new(8);
        let v = p.embed(&[String::new()]).expect("embed").remove(0);
        assert!(v.iter().all(|x| *x == 0.0));
    }
}
