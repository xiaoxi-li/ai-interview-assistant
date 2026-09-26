//! AI 回答生成（IA-08）：面试问题 + 知识库召回 → 提示词 → 流式回答。
//!
//! 提示词构造是纯函数，可完整单测；流式调用由命令层负责（复用 OpenAI 兼容 Provider）。

use crate::interview::recall::RecallHit;
use crate::providers::types::ChatMessage;

/// 面试辅助系统提示词
pub const SYSTEM_PROMPT: &str = "\
你是面试辅助助手。面试官刚刚提出了一个问题，请结合「知识库参考」给出候选人可以直接口述的回答。

要求：
1. 优先采用知识库参考中的项目经历、技术方案与数据，不要编造参考中不存在的公司、项目或数字；
2. 知识库参考不足时，给出通用但专业的回答思路，并在开头标注「（通用思路）」；
3. 先给结论或要点，再按 1. 2. 3. 展开，整体控制在 250 字以内；
4. 使用口语化中文，可直接朗读，不要输出 Markdown 标题、表格或代码块；
5. 不要复述问题，不要输出与回答无关的说明或元信息。";

/// 无召回结果时的占位说明
pub const NO_SOURCE_HINT: &str = "知识库中没有检索到与该问题直接相关的内容";

/// 最多注入的知识库片段数（控制提示词长度与成本）
pub const MAX_CONTEXT_SOURCES: usize = 5;
/// 单条片段摘要的字符上限
pub const MAX_SNIPPET_CHARS: usize = 300;

/// 按字符边界安全截断（超出时追加省略号）
pub fn truncate(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let mut result: String = trimmed.chars().take(max_chars).collect();
    result.push('…');
    result
}

/// 渲染知识库参考上下文（编号 + 标题 + 摘要）
pub fn render_context(sources: &[RecallHit]) -> String {
    if sources.is_empty() {
        return NO_SOURCE_HINT.to_string();
    }

    sources
        .iter()
        .take(MAX_CONTEXT_SOURCES)
        .enumerate()
        .map(|(index, hit)| {
            let title = if hit.title.trim().is_empty() {
                "未命名笔记"
            } else {
                hit.title.trim()
            };
            format!(
                "[{}] {}：{}",
                index + 1,
                title,
                truncate(&hit.snippet, MAX_SNIPPET_CHARS)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 构造用户消息（问题 + 知识库参考 + 输出要求）
pub fn build_user_prompt(question: &str, sources: &[RecallHit]) -> String {
    format!(
        "面试官的问题：\n{}\n\n知识库参考：\n{}\n\n请直接给出回答。",
        question.trim(),
        render_context(sources)
    )
}

/// 构造完整的回答生成消息列表
pub fn build_answer_messages(question: &str, sources: &[RecallHit]) -> Vec<ChatMessage> {
    vec![
        ChatMessage::system(SYSTEM_PROMPT),
        ChatMessage::user(build_user_prompt(question, sources)),
    ]
}

/// 引用来源（去重后落库到 qa_pairs.sources_json，IA-09 据此跳转定位笔记）
pub fn source_refs(sources: &[RecallHit]) -> Vec<crate::interview::store::SourceRef> {
    let mut refs: Vec<crate::interview::store::SourceRef> = Vec::new();
    for hit in sources {
        if hit.note_id.trim().is_empty() {
            continue;
        }
        if refs.iter().any(|existing| existing.note_id == hit.note_id) {
            continue;
        }
        refs.push(crate::interview::store::SourceRef {
            note_id: hit.note_id.clone(),
            title: hit.title.trim().to_string(),
        });
    }
    refs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(title: &str, snippet: &str, score: f32) -> RecallHit {
        RecallHit {
            note_id: format!("n-{title}"),
            title: title.to_string(),
            snippet: snippet.to_string(),
            score,
        }
    }

    #[test]
    fn messages_start_with_system_prompt() {
        let messages = build_answer_messages("你负责过什么项目", &[]);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, crate::providers::types::Role::System);
        assert_eq!(messages[0].content, SYSTEM_PROMPT);
        assert_eq!(messages[1].role, crate::providers::types::Role::User);
    }

    #[test]
    fn user_prompt_contains_question_and_sources() {
        let sources = vec![hit(
            "项目架构设计",
            "负责桌面应用架构与性能优化",
            0.5,
        )];
        let prompt = build_user_prompt("介绍一下你的项目", &sources);

        assert!(prompt.contains("介绍一下你的项目"));
        assert!(prompt.contains("[1] 项目架构设计："));
        assert!(prompt.contains("负责桌面应用架构与性能优化"));
    }

    #[test]
    fn prompt_marks_missing_sources() {
        let prompt = build_user_prompt("介绍一下你的项目", &[]);
        assert!(prompt.contains(NO_SOURCE_HINT));
    }

    #[test]
    fn context_is_limited_to_max_sources() {
        let sources: Vec<RecallHit> = (0..MAX_CONTEXT_SOURCES + 3)
            .map(|index| hit(&format!("笔记{index}"), "内容", 0.1))
            .collect();
        let context = render_context(&sources);

        assert!(context.contains("[5]"));
        assert!(!context.contains("[6]"), "超出上限的片段不应注入：{context}");
    }

    #[test]
    fn long_snippet_is_truncated() {
        let long = "很长的内容".repeat(200);
        let context = render_context(&[hit("长笔记", &long, 0.1)]);
        assert!(context.contains('…'));
        // 标题 + 编号 + 冒号 + 摘要上限 + 省略号
        assert!(context.chars().count() < MAX_SNIPPET_CHARS + 30);
    }

    #[test]
    fn truncate_keeps_short_text_intact() {
        assert_eq!(truncate("  短文本  ", 10), "短文本");
        assert_eq!(truncate("", 10), "");
    }

    #[test]
    fn empty_title_falls_back_to_placeholder() {
        let context = render_context(&[hit("  ", "内容", 0.1)]);
        assert!(context.contains("未命名笔记"));
    }

    #[test]
    fn source_refs_are_deduped_and_keep_note_ids() {
        let sources = vec![
            hit("笔记A", "x", 0.3),
            hit("笔记B", "y", 0.2),
            hit("笔记A", "z", 0.1),
            RecallHit {
                note_id: "  ".into(),
                title: "无 ID".into(),
                snippet: "w".into(),
                score: 0.1,
            },
        ];

        let refs = source_refs(&sources);
        assert_eq!(refs.len(), 2, "应按笔记 ID 去重并跳过空 ID");
        assert_eq!(refs[0].note_id, "n-笔记A");
        assert_eq!(refs[0].title, "笔记A");
        assert_eq!(refs[1].note_id, "n-笔记B");
    }
}
