//! 一键「全链路演示模式」。
//!
//! 目的：让用户**零准备**（无需麦克风、无需下载模型、无需 API Key）就能看到完整链路：
//! `转写 → 问题识别 → 知识库召回 → AI 回答 → 落库 → 会话回放`。
//!
//! 设计原则：**只替换输入源，不替换业务逻辑**。
//! 脚本只提供「面试官/我的发言文本」，问题识别、召回、落库、事件推送全部走
//! 生产代码路径（`QuestionDetector` / `recall_for_question` / `QaRepo` / Tauri 事件），
//! 因此演示即真实链路。唯一例外是无 API Key 时的回答生成走 [`offline_answer`] 离线拼装。

use crate::interview::answer::offline_answer;
use crate::interview::recall::recall_for_question;
use crate::knowledge::store::NotesRepo;
use crate::knowledge::NoteDto;
use crate::providers::EmbeddingProvider;
use crate::providers::types::Speaker;
use rusqlite::Connection;

/// 演示会话的记录数上限（避免脚本无限增长）
pub const MAX_DEMO_TURNS: usize = 24;

/// 演示用知识库笔记（标题 + 正文）
pub struct DemoNote {
    pub title: &'static str,
    pub content: &'static str,
}

/// 演示笔记：内容与脚本中的问题强相关，保证召回一定命中
pub fn demo_notes() -> Vec<DemoNote> {
    vec![
        DemoNote {
            title: "项目经历 · 桌面端架构设计",
            content: "最近三年我主要负责桌面端应用的架构设计：\n\
- 技术栈：Tauri 2 + Rust 后端 + React 18 前端，本地 SQLite 存储\n\
- 分层：命令层 / 领域层 / 存储层；音频、检索、推理等重活全部放在 Rust 侧\n\
- 关键取舍：实时链路放在 Rust 侧，前端只做渲染，避免 IPC 抖动影响交互\n\
- 成果：冷启动 1.2 秒内可交互，常驻内存稳定在 300MB 左右",
        },
        DemoNote {
            title: "性能优化实践",
            content: "性能优化我一般分三步：\n\
1. 量化：先用火焰图与耗时打点定位瓶颈，不凭感觉优化\n\
2. 拆分：把渲染与实时链路解耦，长任务丢到后台线程，主线程只做状态更新\n\
3. 验证：改动前后各跑一轮基准，用数据说话\n\
具体案例：向量检索耗时从 480ms 降到 90ms —— 把 Embedding 计算移出主线程，并为分块结果加了缓存。",
        },
        DemoNote {
            title: "系统设计改进设想",
            content: "如果重新设计这套系统，我会做三件事：\n\
- 传输层：命令式 IPC 换成事件流 + 批量上报，减少往返次数\n\
- 存储层：笔记与向量索引分成两个连接池，写入不再阻塞检索\n\
- 可观测性：第一天就接入结构化日志与指标，而不是事后补",
        },
        DemoNote {
            title: "团队协作方式",
            content: "协作上我主要做两件事：\n\
- 先把接口契约定下来，再并行开发，减少返工\n\
- 每周一次技术对齐，确保各人实现不偏离整体设计\n\
- 代码评审聚焦边界与错误处理，而非风格之争",
        },
    ]
}

/// 演示脚本的一轮发言
pub struct DemoTurn {
    pub speaker: Speaker,
    /// 最终转写文本
    pub text: &'static str,
    /// 中间结果（partial）：真实链路里由 ASR 反复覆盖输出，这里用截断前缀模拟
    pub partial: &'static str,
}

/// 演示脚本：3 轮问答（面试官提问 → 我回答）
pub fn demo_turns() -> Vec<DemoTurn> {
    vec![
        DemoTurn {
            speaker: Speaker::Interviewer,
            text: "先简单介绍一下你自己，还有你最近负责的项目吧。",
            partial: "先简单介绍一下你自己",
        },
        DemoTurn {
            speaker: Speaker::Me,
            text: "好的。我最近三年主要做桌面端应用，负责整体架构设计，把实时链路放在 Rust 侧，前端只做渲染。",
            partial: "好的。我最近三年主要做桌面端应用",
        },
        DemoTurn {
            speaker: Speaker::Interviewer,
            text: "那你在项目里是怎么做性能优化的？能举个具体例子吗？",
            partial: "那你在项目里是怎么做性能优化的",
        },
        DemoTurn {
            speaker: Speaker::Me,
            text: "我一般分三步：先量化瓶颈，再拆分解耦把长任务移到后台线程，最后用改动前后的基准数据验证效果。",
            partial: "我一般分三步",
        },
        DemoTurn {
            speaker: Speaker::Interviewer,
            text: "如果让你重新设计这个系统，你会怎么改进？",
            partial: "如果让你重新设计这个系统",
        },
        DemoTurn {
            speaker: Speaker::Me,
            text: "我会重点改三块：传输层换成事件流、存储层拆分连接池、以及从第一天就补上可观测性。",
            partial: "我会重点改三块",
        },
    ]
}

/// 把文本切成适合「打字机」效果的小块（按字符切分，兼容中文）
pub fn chunk_for_stream(text: &str, chunk_size: usize) -> Vec<String> {
    let size = chunk_size.max(1);
    let chars: Vec<char> = text.chars().collect();
    chars
        .chunks(size)
        .map(|chunk| chunk.iter().collect::<String>())
        .collect()
}

/// 幂等播种演示笔记：已存在同名笔记则跳过；返回本次新增的笔记
pub fn seed_demo_notes(
    conn: &Connection,
    provider: &dyn EmbeddingProvider,
) -> Result<Vec<NoteDto>, String> {
    let repo = NotesRepo::new(conn);
    let existing: Vec<String> = repo
        .list(false)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|note| note.title)
        .collect();

    let mut created = Vec::new();
    for spec in demo_notes() {
        if existing.iter().any(|title| title == spec.title) {
            continue;
        }
        let note = repo
            .create(spec.title, spec.content, None)
            .map_err(|e| e.to_string())?;
        // 向量索引采用「尽力而为」：失败不阻断演示
        if let Err(err) = crate::retrieval::index::reindex_note(conn, provider, &note) {
            tracing::warn!("演示笔记向量索引失败（note={}）：{err}", note.id);
        }
        created.push(note);
    }
    Ok(created)
}

/// 演示回答的生成方式（供事件里标注来源）
pub fn offline_answer_for(question: &str, hits: &[crate::interview::RecallHit]) -> String {
    offline_answer(question, hits)
}

/// 演示脚本 → 知识库召回的纯逻辑自检（供测试与诊断复用）：
/// 依次跑过脚本中的面试官发言，返回 (问题, 召回条数)
pub fn dry_run_recall(
    conn: &Connection,
    provider: &dyn EmbeddingProvider,
) -> Result<Vec<(String, usize)>, String> {
    use crate::interview::question::{
        QuestionDetector, DEFAULT_MERGE_WINDOW_MS, DEFAULT_MIN_CHARS, DEFAULT_THRESHOLD,
    };

    let mut detector = QuestionDetector::new(
        DEFAULT_THRESHOLD,
        DEFAULT_MIN_CHARS,
        DEFAULT_MERGE_WINDOW_MS,
    );
    let mut report = Vec::new();
    let mut ts = 1_000i64;

    for turn in demo_turns().iter().take(MAX_DEMO_TURNS) {
        ts += 2_000;
        if turn.speaker != Speaker::Interviewer {
            continue;
        }
        let Some(question) = detector.push(turn.text, ts) else {
            continue;
        };
        let outcome = recall_for_question(conn, provider, &question.text, crate::interview::DEFAULT_TOP_K)
            .map_err(|e| e.to_string())?;
        report.push((question.text, outcome.hits.len()));
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interview::question::{
        QuestionDetector, DEFAULT_MERGE_WINDOW_MS, DEFAULT_MIN_CHARS, DEFAULT_THRESHOLD,
    };
    use crate::providers::embedding::MockEmbeddingProvider;
    use crate::storage::Db;

    fn detector() -> QuestionDetector {
        QuestionDetector::new(
            DEFAULT_THRESHOLD,
            DEFAULT_MIN_CHARS,
            DEFAULT_MERGE_WINDOW_MS,
        )
    }

    #[test]
    fn demo_script_alternates_speakers_and_has_questions() {
        let turns = demo_turns();
        assert!(turns.len() >= 6, "演示脚本至少 3 轮问答");
        assert_eq!(turns.len() % 2, 0, "面试官与我的发言应成对出现");

        for (index, turn) in turns.iter().enumerate() {
            let expected = if index % 2 == 0 {
                Speaker::Interviewer
            } else {
                Speaker::Me
            };
            assert_eq!(turn.speaker, expected, "第 {} 轮说话人不符", index + 1);
            assert!(!turn.text.trim().is_empty());
            assert!(
                turn.text.starts_with(turn.partial),
                "partial 应是 final 的前缀：{} / {}",
                turn.partial,
                turn.text
            );
        }
    }

    #[test]
    fn demo_script_questions_are_all_detected() {
        let mut detector = detector();
        let mut ts = 0i64;
        let mut detected = 0;

        for turn in demo_turns() {
            ts += 2_000;
            if turn.speaker != Speaker::Interviewer {
                continue;
            }
            if detector.push(turn.text, ts).is_some() {
                detected += 1;
            }
        }

        assert_eq!(detected, 3, "三段面试官发言都应被识别为问题");
    }

    #[test]
    fn demo_notes_are_unique_and_non_empty() {
        let notes = demo_notes();
        assert!(notes.len() >= 3);
        for note in &notes {
            assert!(!note.title.trim().is_empty());
            assert!(note.content.chars().count() > 30, "笔记正文应足够形成召回片段");
        }
        for (index, note) in notes.iter().enumerate() {
            for other in notes.iter().skip(index + 1) {
                assert_ne!(note.title, other.title, "演示笔记标题不应重复");
            }
        }
    }

    #[test]
    fn seed_is_idempotent_and_searchable() {
        let db = Db::open_in_memory().expect("db");
        let provider = MockEmbeddingProvider::default();

        let first = seed_demo_notes(db.conn(), &provider).expect("seed");
        assert_eq!(first.len(), demo_notes().len());

        let second = seed_demo_notes(db.conn(), &provider).expect("seed again");
        assert!(second.is_empty(), "重复播种不应新建笔记");

        // 播种后应能被关键词检索命中
        let hits = crate::retrieval::search_notes(db.conn(), "性能优化", 5).expect("search");
        assert!(!hits.is_empty(), "演示笔记应可被检索");
    }

    /// 端到端（离线）：脚本 → 问题识别 → 真实召回 → 离线回答内容包含召回笔记
    #[test]
    fn demo_full_chain_works_offline() {
        let db = Db::open_in_memory().expect("db");
        let provider = MockEmbeddingProvider::default();
        seed_demo_notes(db.conn(), &provider).expect("seed");

        let report = dry_run_recall(db.conn(), &provider).expect("dry run");
        assert_eq!(report.len(), 3, "三段面试官发言都应产出问题");

        for (question, hit_count) in &report {
            assert!(*hit_count > 0, "问题「{question}」应召回到笔记");
        }

        // 离线回答应包含召回笔记的标题与内容
        let hits = recall_for_question(db.conn(), &provider, &report[0].0, 3)
            .expect("recall")
            .hits;
        let answer = offline_answer_for(&report[0].0, &hits);
        assert!(answer.contains(&report[0].0), "回答应回应问题本身");
        assert!(answer.contains(&hits[0].title), "回答应包含引用笔记标题");
        assert!(answer.contains("离线"), "无 Key 时应明确标注为离线演示回答");
    }

    #[test]
    fn offline_answer_handles_empty_hits() {
        let answer = offline_answer("一个知识库里没有的问题", &[]);
        assert!(answer.contains("没有召回到"));
        assert!(answer.contains("一个知识库里没有的问题"));
    }

    #[test]
    fn chunk_for_stream_reassembles_text() {
        let text = "我一般分三步：先量化瓶颈，再拆分解耦，最后用基准数据验证。";
        for size in [1usize, 3, 7, 64] {
            let chunks = chunk_for_stream(text, size);
            assert!(!chunks.is_empty());
            assert_eq!(chunks.concat(), text, "分块应可无损还原（size={size}）");
        }
        assert!(chunk_for_stream("", 4).is_empty());
        assert_eq!(chunk_for_stream("abc", 0), vec!["a", "b", "c"]);
    }
}
