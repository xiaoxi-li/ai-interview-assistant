//! Markdown 解析：标题提取与路径判定。

use std::path::Path;

/// 支持的 Markdown 扩展名
pub const MARKDOWN_EXTENSIONS: [&str; 2] = ["md", "markdown"];

/// 判断路径是否为 Markdown 文件（忽略大小写）
pub fn is_markdown_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| {
            MARKDOWN_EXTENSIONS
                .iter()
                .any(|m| ext.eq_ignore_ascii_case(m))
        })
        .unwrap_or(false)
}

/// 去除 UTF-8 BOM
pub fn strip_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

/// 提取标题：取首个合法 ATX 标题（`# ` ~ `###### `），
/// 形式不合法或无标题时回退到 `fallback`（通常为文件名）。
pub fn extract_title(content: &str, fallback: &str) -> String {
    for line in strip_bom(content).lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('#') {
            continue;
        }
        let hashes = trimmed.chars().take_while(|c| *c == '#').count();
        if hashes == 0 || hashes > 6 {
            continue;
        }
        let after = &trimmed[hashes..];
        // ATX 标题要求 # 后至少一个空白字符
        if !after.starts_with(' ') && !after.starts_with('\t') {
            continue;
        }
        // 允许 `## 标题 ##` 的闭合写法
        let title = after.trim().trim_end_matches('#').trim();
        if !title.is_empty() {
            return title.to_string();
        }
    }
    fallback.to_string()
}

/// 解析结果
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedMarkdown {
    pub title: String,
    pub content: String,
}

/// 解析 Markdown 文本
pub fn parse(content: &str, fallback_title: &str) -> ParsedMarkdown {
    let content = strip_bom(content).to_string();
    let title = extract_title(&content, fallback_title);
    ParsedMarkdown { title, content }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn extracts_h1_title() {
        assert_eq!(extract_title("# 项目架构\n正文", "fallback"), "项目架构");
    }

    #[test]
    fn extracts_first_heading_of_any_level() {
        assert_eq!(extract_title("前言\n## 二级标题\n", "fallback"), "二级标题");
    }

    #[test]
    fn tolerates_closing_hashes() {
        assert_eq!(extract_title("## 带闭合 ##", "fallback"), "带闭合");
    }

    #[test]
    fn ignores_hash_without_space() {
        assert_eq!(extract_title("#无空格\n正文", "fallback"), "fallback");
    }

    #[test]
    fn falls_back_when_no_heading() {
        assert_eq!(extract_title("只有正文\n没有标题", "文件名"), "文件名");
    }

    #[test]
    fn strips_bom_before_parsing() {
        let parsed = parse("\u{feff}# 带 BOM\n内容", "fallback");
        assert_eq!(parsed.title, "带 BOM");
        assert!(!parsed.content.starts_with('\u{feff}'));
    }

    #[test]
    fn detects_markdown_paths_case_insensitively() {
        assert!(is_markdown_path(&PathBuf::from("a.md")));
        assert!(is_markdown_path(&PathBuf::from("a.MARKDOWN")));
        assert!(!is_markdown_path(&PathBuf::from("a.txt")));
        assert!(!is_markdown_path(&PathBuf::from("noext")));
    }
}
