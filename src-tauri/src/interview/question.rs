//! 面试官问题识别。
//!
//! 规则式判定（无额外模型、零延迟、可解释），命中信号会在界面上展示，便于用户判断。
//! 语音转写没有标点时主要依赖疑问词与祈使式提问（「介绍一下…」「说说…」）。
//!
//! **判定单位是「句子」而非「整段」**：一条转写（尤其云端按句返回的 final）常包含多句话，
//! 若拿整段做判定，句中问号/句尾语气词会被后面的话稀释而漏判（2026-09-28 真实踩坑，见
//! [`analyze_with`] 注释）。因此先按句末标点切句、逐句判定，再取最高分作为整段结论。
//!
//! 两个入口：
//! * [`analyze`]：对单段文本打分（纯函数）；
//! * [`QuestionDetector`]：跨段聚合（面试官常中途停顿分词），并支持手动触发。

use serde::{Deserialize, Serialize};

/// 默认判定阈值
pub const DEFAULT_THRESHOLD: f32 = 0.5;
/// 默认最短字符数（过滤「嗯？」这类碎片）
pub const DEFAULT_MIN_CHARS: usize = 4;
/// 默认聚合时间窗（毫秒）：窗内的连续转写会合并成一个候选问题
pub const DEFAULT_MERGE_WINDOW_MS: i64 = 3_000;
/// 缓冲最长存活时间：超时且未判定为问题则丢弃，避免无限增长
pub const DEFAULT_STALE_MS: i64 = 10_000;
/// 手动触发时回溯的历史窗口
pub const DEFAULT_MANUAL_WINDOW_MS: i64 = 30_000;

/// 疑问句尾标点（强信号）
const QUESTION_MARKS: &[&str] = &["？", "?"];

/// 疑问词
const INTERROGATIVES: &[&str] = &[
    "什么",
    "怎么",
    "为什么",
    "为何",
    "如何",
    "哪一",
    "哪个",
    "哪些",
    "哪里",
    "谁",
    "多少",
    "几个",
    "多久",
    "是否",
    "有没有",
    "能不能",
    "能否",
    "可不可以",
    "是不是",
];

/// 祈使式提问（面试场景高频：「介绍一下你的项目」）
const IMPERATIVES: &[&str] = &[
    "介绍一下",
    "说说",
    "讲讲",
    "谈谈",
    "聊聊",
    "描述一下",
    "举个例子",
    "详细讲讲",
    "展开讲讲",
    "说一下你的",
    "谈谈你的",
];

/// 第二人称疑问结构
const PATTERNS: &[&str] = &[
    "你觉得",
    "你认为",
    "你会怎么",
    "你是怎么",
    "你遇到过",
    "你怎么看",
    "你的理解",
    "你的思路",
];

/// 句尾语气词
const TAIL_PARTICLES: &[&str] = &["吗", "呢"];

/// 纯应答/语气词（整段命中即判为非问题）
const FILLERS: &[&str] = &[
    "嗯",
    "嗯嗯",
    "哦",
    "噢",
    "好的",
    "好",
    "明白",
    "收到",
    "对",
    "是的",
    "行",
    "ok",
    "OK",
    "谢谢",
    "辛苦了",
    "没问题",
];

/// 单段文本的问题判定结果
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionScore {
    pub is_question: bool,
    pub score: f32,
    /// 命中的可读信号（用于界面展示与调试）
    pub signals: Vec<String>,
}

impl QuestionScore {
    fn not_question(signals: Vec<String>) -> Self {
        Self {
            is_question: false,
            score: 0.0,
            signals,
        }
    }
}

/// 去掉首尾空白与标点，便于关键词匹配
fn normalize(text: &str) -> String {
    text.trim()
        .trim_matches(|c: char| {
            matches!(
                c,
                '。' | '，' | ',' | '!' | '！' | '~' | '、' | ' ' | '.' | '…' | '？' | '?' | '；' | ';'
            )
        })
        .to_string()
}

/// 完全去除空白与标点（用于语气词判定、去重比较）
fn strip_punctuation(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_whitespace() && !"，。？?！!、,~…：:；;".contains(*c))
        .collect()
}

/// 使用默认阈值判定
pub fn analyze(text: &str) -> QuestionScore {
    analyze_with(text, DEFAULT_THRESHOLD)
}

/// 按给定阈值判定文本是否为面试官问题
///
/// 权重设计：任一「强信号」（问号 / 疑问词 / 祈使式提问）单独即可越过默认阈值 0.5；
/// 「第二人称疑问」「句尾语气词」属弱信号，需与强信号组合。
///
/// 判定按**句**进行（见 [`split_sentences`]）并取最高分：转写结果常在一条文本里包含多句话，
/// 典型踩坑（2026-09-28）：`呃第一个问题，lvm了解过吗？假如你有块新的磁盘…你说一下用到的命令。`
/// ——问号在句中、句尾是句号，整段判定得分 0，问题被静默丢弃；按句判定则首句命中。
pub fn analyze_with(text: &str, threshold: f32) -> QuestionScore {
    split_sentences(text)
        .into_iter()
        .map(|sentence| analyze_sentence(sentence, threshold))
        .reduce(|best, current| if current.score > best.score { current } else { best })
        .unwrap_or_else(|| QuestionScore::not_question(vec![]))
}

/// 句末分隔符（中英文）：用于把一条转写切成句子
const SENTENCE_BREAKS: &[char] = &['。', '！', '？', '!', '?', '；', ';', '\n'];

/// 切句：句末标点保留在句尾；末段（可能尚未说完）原样保留
fn split_sentences(text: &str) -> Vec<&str> {
    let mut sentences = Vec::new();
    let mut start = 0usize;
    for (index, ch) in text.char_indices() {
        if SENTENCE_BREAKS.contains(&ch) {
            let end = index + ch.len_utf8();
            sentences.push(&text[start..end]);
            start = end;
        }
    }
    if start < text.len() {
        sentences.push(&text[start..]);
    }
    sentences
}

/// 单句判定（问号 / 疑问词 / 祈使式提问 / 第二人称疑问 / 句尾语气词）
fn analyze_sentence(text: &str, threshold: f32) -> QuestionScore {
    let trimmed = text.trim();
    let bare = strip_punctuation(trimmed);
    if bare.is_empty() {
        return QuestionScore::not_question(vec![]);
    }

    // 纯应答/语气词直接排除（即使带问号，如「嗯？」）
    if FILLERS
        .iter()
        .any(|filler| bare.to_lowercase() == filler.to_lowercase())
    {
        return QuestionScore::not_question(vec!["语气词/应答".to_string()]);
    }

    let body = normalize(trimmed);
    let mut score = 0.0f32;
    let mut signals = Vec::new();

    if QUESTION_MARKS.iter().any(|mark| trimmed.ends_with(mark)) {
        score += 0.6;
        signals.push("以问号结尾".to_string());
    }

    if let Some(word) = INTERROGATIVES.iter().find(|word| body.contains(**word)) {
        score += 0.55;
        signals.push(format!("含疑问词「{word}」"));
    }

    if let Some(word) = IMPERATIVES.iter().find(|word| body.contains(**word)) {
        score += 0.55;
        signals.push(format!("祈使式提问「{word}」"));
    }

    if let Some(particle) = TAIL_PARTICLES.iter().find(|p| body.ends_with(**p)) {
        score += 0.4;
        signals.push(format!("句尾语气词「{particle}」"));
    }

    if let Some(pattern) = PATTERNS.iter().find(|pattern| body.contains(**pattern)) {
        score += 0.45;
        signals.push(format!("第二人称疑问「{pattern}」"));
    }

    QuestionScore {
        is_question: score >= threshold,
        score: (score * 100.0).round() / 100.0,
        signals,
    }
}

/// 识别出的问题
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedQuestion {
    pub text: String,
    pub score: f32,
    pub signals: Vec<String>,
    /// 判定时刻（毫秒）
    pub ts: i64,
}

/// 跨段聚合检测器
pub struct QuestionDetector {
    threshold: f32,
    min_chars: usize,
    merge_window_ms: i64,
    stale_ms: i64,
    manual_window_ms: i64,

    /// 当前正在累积的候选文本
    buffer: String,
    buffer_start: i64,
    buffer_end: i64,
    /// 最近的历史转写（供手动触发回溯），元素为 (ts, text)
    history: Vec<(i64, String)>,
    /// 上一次已产出的问题（用于跨段去重）
    last_emitted: Option<String>,
}

impl QuestionDetector {
    pub fn new(threshold: f32, min_chars: usize, merge_window_ms: i64) -> Self {
        Self {
            threshold,
            min_chars: min_chars.max(1),
            merge_window_ms: merge_window_ms.max(1),
            stale_ms: DEFAULT_STALE_MS,
            manual_window_ms: DEFAULT_MANUAL_WINDOW_MS,
            buffer: String::new(),
            buffer_start: 0,
            buffer_end: 0,
            history: Vec::new(),
            last_emitted: None,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(DEFAULT_THRESHOLD, DEFAULT_MIN_CHARS, DEFAULT_MERGE_WINDOW_MS)
    }

    /// 当前累积的候选文本（界面可用于展示「正在识别」）
    pub fn pending_text(&self) -> &str {
        &self.buffer
    }

    /// 送入一段面试官转写；命中问题时返回结果
    pub fn push(&mut self, text: &str, ts: i64) -> Option<DetectedQuestion> {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }

        self.history.push((ts, text.to_string()));
        let cutoff = ts - self.manual_window_ms;
        self.history.retain(|(history_ts, _)| *history_ts >= cutoff);

        // 与上一段超出聚合窗口：旧缓冲先结算。
        // 结算出问题则本轮到此为止（新文本留待下次）；未成问题则丢弃并继续评估新文本。
        if !self.buffer.is_empty() && ts - self.buffer_end > self.merge_window_ms {
            let settled = self.evaluate();
            if settled.is_some() {
                self.append(text, ts);
                return settled;
            }
            self.reset();
        }

        // 长时间没有任何疑问特征的陈述：丢弃，避免缓冲无限增长
        if !self.buffer.is_empty() && ts - self.buffer_start > self.stale_ms {
            // 只记录长度与结论，不落正文（原始转写属用户内容，日志只用于定位「为何没识别到」）
            tracing::debug!(
                "问题候选缓冲超时丢弃（{} 字，未命中疑问特征）",
                self.buffer.chars().count()
            );
            self.reset();
        }

        self.append(text, ts);
        self.evaluate()
    }

    /// 手动触发：用最近窗口内的转写做一次判定（即使分数偏低也返回，交由用户决定）
    pub fn trigger(&mut self, ts: i64) -> Option<DetectedQuestion> {
        // 缓冲过旧则忽略，改由历史记录回溯
        let mut candidate = if !self.buffer.is_empty() && ts - self.buffer_end <= self.manual_window_ms
        {
            self.buffer.clone()
        } else {
            String::new()
        };
        let cutoff = ts - self.manual_window_ms;

        if candidate.is_empty() {
            let recent: Vec<&str> = self
                .history
                .iter()
                .filter(|(history_ts, _)| *history_ts >= cutoff)
                .map(|(_, text)| text.as_str())
                .collect();
            candidate = recent.join(" ");
        }

        let candidate = candidate.trim().to_string();
        if candidate.chars().count() < self.min_chars {
            return None;
        }

        let score = analyze_with(&candidate, self.threshold);
        self.reset();
        self.last_emitted = Some(candidate.clone());

        Some(DetectedQuestion {
            text: candidate,
            score: score.score,
            signals: score.signals,
            ts,
        })
    }

    /// 音频结束：结算剩余缓冲
    pub fn flush(&mut self) -> Option<DetectedQuestion> {
        self.evaluate()
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
        self.buffer_start = 0;
        self.buffer_end = 0;
    }

    /// 清空全部状态（含历史与去重记录）
    pub fn clear(&mut self) {
        self.reset();
        self.history.clear();
        self.last_emitted = None;
    }

    fn append(&mut self, text: &str, ts: i64) {
        if self.buffer.is_empty() {
            self.buffer_start = ts;
        } else if !self.buffer.ends_with(' ') {
            self.buffer.push(' ');
        }
        self.buffer.push_str(text);
        self.buffer_end = ts;
    }

    /// 评价当前缓冲；命中则产出并清空
    fn evaluate(&mut self) -> Option<DetectedQuestion> {
        if self.buffer.is_empty() {
            return None;
        }
        if self.buffer.chars().count() < self.min_chars {
            return None;
        }

        let score = analyze_with(&self.buffer, self.threshold);
        if !score.is_question {
            return None;
        }

        let text = self.buffer.trim().to_string();
        if self
            .last_emitted
            .as_deref()
            .is_some_and(|previous| is_similar(previous, &text))
        {
            // 同一问题的续段：合并后已产出过，不重复上报
            self.reset();
            return None;
        }

        self.last_emitted = Some(text.clone());
        let ts = self.buffer_end;
        self.reset();
        Some(DetectedQuestion {
            text,
            score: score.score,
            signals: score.signals,
            ts,
        })
    }
}

/// 归一化后比较：完全相同、或一方是另一方的前缀（应对中途停顿被切段）
fn is_similar(a: &str, b: &str) -> bool {
    let (left, right) = (strip_punctuation(a), strip_punctuation(b));
    if left.is_empty() || right.is_empty() {
        return false;
    }
    left == right || left.starts_with(&right) || right.starts_with(&left)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_question(text: &str) -> bool {
        analyze(text).is_question
    }

    #[test]
    fn question_mark_is_a_strong_signal() {
        let score = analyze("这个项目的架构是怎样的？");
        assert!(score.is_question);
        assert!(score.score >= 0.6);
        assert!(score.signals.iter().any(|s| s.contains("问号")));
    }

    #[test]
    fn interrogative_words_detected() {
        assert!(is_question("你当时怎么解决这个问题的"));
        assert!(is_question("为什么选择这个方案"));
        assert!(is_question("你负责哪一部分"));
    }

    #[test]
    fn imperative_interview_prompts_count_as_questions() {
        // 面试官高频：无标点、无疑问词的祈使式提问
        assert!(is_question("介绍一下你最近做的项目"));
        assert!(is_question("说说你的技术栈"));
        assert!(is_question("举个例子说明一下"));
    }

    #[test]
    fn second_person_patterns_detected() {
        let score = analyze("你觉得这个方案有什么风险");
        assert!(score.is_question);
        assert!(score.signals.iter().any(|s| s.contains("第二人称")));
    }

    #[test]
    fn statements_are_not_questions() {
        assert!(!is_question("我负责后端开发"));
        assert!(!is_question("我们团队一共五个人"));
        assert!(!is_question("这个项目做了三个月"));
    }

    #[test]
    fn statement_only_multi_sentence_text_is_not_question() {
        assert!(!is_question("第一句是陈述。第二句也是陈述。"));
    }

    #[test]
    fn multi_sentence_segment_with_inner_question_is_detected() {
        // 真实踩坑（2026-09-28，云端 ASR 按句返回）：一条转写含多句话、问号在句中，
        // 整段判定得分 0 → 问题被静默丢弃；按句判定后首句应命中。
        let score = analyze(
            "呃第一个问题，lvm了解过吗？假如你有你有块新的磁盘，你要用lvm的方式挂载到指定的目录，你说一下这个过程中用到的命令。",
        );
        assert!(score.is_question, "句中问号应命中：{score:?}");
        assert!(score.signals.iter().any(|signal| signal.contains("问号")));
    }

    #[test]
    fn fillers_are_never_questions() {
        for filler in ["嗯", "好的", "明白", "OK", "谢谢", "嗯？"] {
            let score = analyze(filler);
            assert!(!score.is_question, "{filler} 不应判为问题");
        }
    }

    #[test]
    fn empty_or_punctuation_only_text_is_not_question() {
        for text in ["", "   ", "。", "，"] {
            assert!(!analyze(text).is_question, "{text:?} 不应判为问题");
        }
    }

    #[test]
    fn stronger_signals_score_higher() {
        let weak = analyze("哪一部分").score;
        let strong = analyze("你觉得哪一部分是重点？").score;
        assert!(strong > weak, "strong={strong} weak={weak}");
    }

    #[test]
    fn score_is_rounded_to_two_decimals() {
        let score = analyze("为什么这样做").score;
        assert_eq!(score, (score * 100.0).round() / 100.0);
    }

    // ---------- QuestionDetector ----------

    #[test]
    fn detector_emits_on_direct_question() {
        let mut detector = QuestionDetector::with_defaults();
        let detected = detector
            .push("介绍一下你最近的项目", 1_000)
            .expect("应识别为问题");
        assert_eq!(detected.text, "介绍一下你最近的项目");
        assert!(detected.ts == 1_000);
        assert_eq!(detector.pending_text(), "", "产出后应清空缓冲");
    }

    #[test]
    fn detector_keeps_accumulating_until_question_forms() {
        let mut detector = QuestionDetector::with_defaults();
        // 面试官中途停顿分词：前半段不是问题
        assert!(detector.push("我们团队用的是微服务", 1_000).is_none());
        assert!(detector.push("你觉得", 1_500).is_none());
        // 后半段补全后形成疑问
        let detected = detector.push("这种架构有什么优缺点", 2_000).expect("应识别");
        assert!(detected.text.contains("微服务"));
        assert!(detected.text.contains("优缺点"));
    }

    #[test]
    fn detector_does_not_repeat_same_question() {
        let mut detector = QuestionDetector::with_defaults();
        assert!(detector.push("介绍一下你的项目", 1_000).is_some());
        // 续段重复内容不应重复上报
        assert!(detector.push("介绍一下你的项目吧", 1_200).is_none());
    }

    #[test]
    fn detector_settles_previous_buffer_when_window_passed() {
        let mut detector = QuestionDetector::with_defaults();
        assert!(detector.push("你觉得", 1_000).is_none());
        // 相隔超过聚合窗口：旧缓冲「你觉得…」单独结算（含第二人称疑问 → 命中）
        let detected = detector.push("介绍一下项目", 10_000);
        assert!(detected.is_some(), "超窗应先结算旧候选");
    }

    #[test]
    fn detector_drops_stale_statement_buffer() {
        let mut detector = QuestionDetector::with_defaults();
        assert!(detector.push("我负责后端开发", 1_000).is_none());
        // 超过 stale 窗口后缓冲被丢弃
        assert!(detector.push("包括网关和鉴权", 20_000).is_none());
        assert!(!detector.pending_text().contains("我负责后端开发"));
    }

    #[test]
    fn detector_ignores_too_short_fragment() {
        let mut detector = QuestionDetector::with_defaults();
        // 「哪」单字不足 min_chars，不产出
        assert!(detector.push("哪", 1_000).is_none());
    }

    #[test]
    fn flush_settles_pending_question() {
        let mut detector = QuestionDetector::with_defaults();
        assert!(detector.push("你遇到过这种情况吗", 5_000).is_some());
        // 已产出后 flush 不应重复
        assert!(detector.flush().is_none());
    }

    #[test]
    fn manual_trigger_uses_recent_history() {
        let mut detector = QuestionDetector::with_defaults();
        detector.push("我负责后端", 1_000);
        detector.push("大概三年经验", 2_000);

        let detected = detector.trigger(3_000).expect("手动触发应返回候选");
        assert!(detected.text.contains("我负责后端"));
        assert!(detected.text.contains("大概三年经验"));
        // 手动触发后缓冲被清空
        assert_eq!(detector.pending_text(), "");
    }

    #[test]
    fn manual_trigger_returns_none_without_content() {
        let mut detector = QuestionDetector::with_defaults();
        assert!(detector.trigger(1_000).is_none());
    }

    #[test]
    fn manual_trigger_ignores_history_outside_window() {
        let mut detector = QuestionDetector::with_defaults();
        detector.push("很久以前的一段话", 1_000);
        // 超出手动回溯窗口（30s）后不应再被使用
        assert!(detector.trigger(60_000).is_none());
    }

    #[test]
    fn clear_resets_history_and_dedupe() {
        let mut detector = QuestionDetector::with_defaults();
        assert!(detector.push("介绍一下你的项目", 1_000).is_some());
        detector.clear();
        // 清空后同样的问题可以再次被识别
        assert!(detector.push("介绍一下你的项目", 2_000).is_some());
    }

    #[test]
    fn detector_emits_multi_sentence_segment_and_keeps_working() {
        let mut detector = QuestionDetector::with_defaults();
        let text = "呃第一个问题，lvm了解过吗？假如你有块新的磁盘，你要用lvm的方式挂载到指定的目录，你说一下这个过程中用到的命令。";

        let detected = detector.push(text, 1_000).expect("内层疑问句应被识别");
        assert_eq!(detected.text, text, "整段文本应作为问题上报");
        assert_eq!(detector.pending_text(), "", "上报后应清空缓冲");

        // 后续问题不受影响（不因去重逻辑被吞掉）
        let next = detector
            .push("要改服务器的SS端口的话，要去哪个文件上去修改哪个参数？", 39_000)
            .expect("下一句问题应被识别");
        assert!(next.text.contains("SS端口"));
    }
}
