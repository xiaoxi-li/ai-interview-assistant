//! 文本分块：优先按段落切分，超长段落再按字符窗口切分并保留重叠。

/// 单块目标字符数（中文场景下约等于数百 token）
pub const DEFAULT_CHUNK_CHARS: usize = 400;
/// 相邻块的重叠字符数，避免语义在边界被截断
pub const DEFAULT_CHUNK_OVERLAP: usize = 50;

/// 将正文切分为带重叠的分块
pub fn chunk_text(content: &str, chunk_chars: usize, overlap: usize) -> Vec<String> {
    let chunk_chars = chunk_chars.max(1);
    let overlap = overlap.min(chunk_chars.saturating_sub(1));

    let mut chunks = Vec::new();
    for paragraph in content.split("\n\n") {
        let trimmed = paragraph.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.chars().count() <= chunk_chars {
            chunks.push(trimmed.to_string());
            continue;
        }
        chunks.extend(split_long_text(trimmed, chunk_chars, overlap));
    }
    chunks
}

/// 按字符窗口切分超长文本，窗口之间保留 `overlap` 个字符
fn split_long_text(text: &str, chunk_chars: usize, overlap: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let step = chunk_chars.saturating_sub(overlap).max(1);

    let mut result = Vec::new();
    let mut start = 0usize;
    while start < chars.len() {
        let end = (start + chunk_chars).min(chars.len());
        let piece: String = chars[start..end].iter().collect();
        let piece = piece.trim();
        if !piece.is_empty() {
            result.push(piece.to_string());
        }
        if end >= chars.len() {
            break;
        }
        start += step;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_paragraph_is_single_chunk() {
        let chunks = chunk_text("第一段\n\n第二段", DEFAULT_CHUNK_CHARS, DEFAULT_CHUNK_OVERLAP);
        assert_eq!(chunks, vec!["第一段", "第二段"]);
    }

    #[test]
    fn empty_content_yields_no_chunks() {
        assert!(chunk_text("", 400, 50).is_empty());
        assert!(chunk_text("   \n\n  ", 400, 50).is_empty());
    }

    #[test]
    fn long_paragraph_is_split_with_overlap() {
        let text: String = "abcdefghij".repeat(10); // 100 字符
        let chunks = chunk_text(&text, 30, 10);
        assert!(chunks.len() > 1, "超长段落应被切分");
        assert!(chunks.iter().all(|c| c.chars().count() <= 30));

        // 相邻块之间应有重叠：前块尾部 10 字符 = 后块头部 10 字符
        let first_tail: String = chunks[0]
            .chars()
            .rev()
            .take(10)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        assert!(chunks[1].starts_with(&first_tail), "相邻块应保留重叠");
    }

    #[test]
    fn overlap_is_clamped_below_chunk_size() {
        // overlap >= chunk_size 时不应死循环
        let chunks = chunk_text(&"x".repeat(50), 10, 20);
        assert!(!chunks.is_empty());
    }

    #[test]
    fn mixed_paragraphs_are_all_covered() {
        let content = format!("短段\n\n{}\n\n尾段", "长文本".repeat(100));
        let chunks = chunk_text(&content, 50, 5);
        assert_eq!(chunks.first().map(String::as_str), Some("短段"));
        assert_eq!(chunks.last().map(String::as_str), Some("尾段"));
    }
}
