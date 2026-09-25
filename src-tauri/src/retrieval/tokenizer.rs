//! 中文分词：FTS5 默认 unicode61 分词器无法切分中文，
//! 因此在写入索引前用 jieba 预分词，并以空格拼接词元。

use jieba_rs::Jieba;
use std::sync::OnceLock;

fn jieba() -> &'static Jieba {
    static JIEBA: OnceLock<Jieba> = OnceLock::new();
    JIEBA.get_or_init(Jieba::new)
}

/// 分词并返回词元列表。
///
/// 使用 `cut_for_search`：在细粒度切分之外额外产出复合词（如"架构设计"同时给出
/// "架构"与"设计"），保证查询侧的子词也能命中索引侧的复合词。
pub fn tokens(text: &str) -> Vec<String> {
    if text.trim().is_empty() {
        return Vec::new();
    }
    jieba()
        .cut_for_search(text, true)
        .into_iter()
        .map(|t| t.to_string())
        .collect()
}

/// 预分词为以空格分隔的索引文本
pub fn tokenize(text: &str) -> String {
    tokens(text).join(" ")
}

/// 是否为有检索意义的词元（包含字母、数字或汉字）
pub fn is_meaningful(token: &str) -> bool {
    token.chars().any(|c| c.is_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_chinese_into_words() {
        let result = tokens("项目架构设计");
        assert!(result.len() > 1, "中文应被切分为多个词元：{result:?}");
        assert!(result.contains(&"架构".to_string()), "应包含「架构」：{result:?}");
    }

    #[test]
    fn keeps_english_words_intact() {
        let result = tokens("event loop model");
        assert!(result.contains(&"event".to_string()));
        assert!(result.contains(&"loop".to_string()));
    }

    #[test]
    fn tokenize_returns_space_separated_tokens() {
        let joined = tokenize("向量检索");
        let parts: Vec<&str> = joined.split_whitespace().collect();
        assert!(parts.len() > 1, "中文应被切分为多个词元：{joined}");
        assert_eq!(parts.len(), tokens("向量检索").len());
    }

    #[test]
    fn empty_input_yields_empty_output() {
        assert!(tokenize("").is_empty());
        assert!(tokenize("   ").is_empty());
        assert!(tokens("").is_empty());
    }

    #[test]
    fn meaningful_filter_rejects_punctuation() {
        assert!(is_meaningful("架构"));
        assert!(is_meaningful("rust2024"));
        assert!(!is_meaningful("，。")); 
        assert!(!is_meaningful("!!"));
    }
}
