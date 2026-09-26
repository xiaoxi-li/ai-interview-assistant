/**
 * 轻量 i18n：**以中文原文作为键**，英文词典按需翻译。
 *
 * 设计理由：
 * 1. 未翻译的文案会自动回落为中文原文，界面永不出现空串或 key 泄漏；
 * 2. 迁移成本极低（把已有中文串直接包进 `t("...")` 即可），可增量补齐英文；
 * 3. 无第三方依赖，包体零增长。
 *
 * 用法：`const t = useT(); t("开始转写")` / `t("已识别 {count} 个问题", { count })`
 */

export type Locale = "zh-CN" | "en-US";

export const LOCALES: { value: Locale; label: string }[] = [
  { value: "zh-CN", label: "简体中文" },
  { value: "en-US", label: "English" },
];

export const DEFAULT_LOCALE: Locale = "zh-CN";

/** 本地持久化键名 */
const STORAGE_KEY = "aia.locale";

/** 英文词典：中文原文 → 英文文案 */
const EN: Record<string, string> = {
  // 应用外壳
  "AI 面试助手": "AI Interview Assistant",
  知识库: "Knowledge",
  面试辅助: "Interview",
  会话历史: "Sessions",
  设置: "Settings",
  最小化: "Minimize",
  最大化: "Maximize",
  关闭: "Close",
  "迷你模式（仅显示当前问题与 AI 回答）": "Mini mode (question & answer only)",

  // 面试页
  未开始捕获: "Not capturing",
  已暂停采集: "Capture paused",
  正在转写面试官: "Transcribing interviewer",
  正在转写我的语音: "Transcribing my voice",
  快捷操作: "Quick actions",
  一键提问: "Ask now",
  "一键提问（{keys}）": "Ask now ({keys})",
  暂停采集: "Pause capture",
  继续采集: "Resume capture",
  "暂停采集（{keys}）": "Pause capture ({keys})",
  "继续采集（{keys}）": "Resume capture ({keys})",
  重新生成: "Regenerate",
  "重新生成回答（{keys}）": "Regenerate answer ({keys})",
  复制回答: "Copy answer",
  "复制回答（{keys}）": "Copy answer ({keys})",
  置顶: "On top",
  "隐身模式：窗口置顶": "Stealth: keep window on top",
  防录屏: "Hide from capture",
  "隐身模式：屏幕共享时隐藏窗口内容（防录屏）":
    "Stealth: hide window content during screen sharing",

  // 实时转写
  实时转写: "Live transcript",
  已停止: "Stopped",
  转写中: "Transcribing",
  双通道转写中: "Dual-channel transcribing",
  清空当前转写显示: "Clear transcript view",
  双通道: "Dual channel",
  "双通道（区分说话人）": "Dual channel (speaker separation)",
  "面试官（系统音频）": "Interviewer (system audio)",
  "我（麦克风）": "Me (microphone)",
  开始转写: "Start transcription",
  停止转写: "Stop transcription",
  面试官: "Interviewer",
  我: "Me",
  "暂无转写内容。": "No transcript yet.",
  "双通道模式可同时转写「我」与「面试官」（音频不出本机）。":
    "Dual-channel mode transcribes both you and the interviewer; audio never leaves this device.",
  "回声抑制 {db}dB": "Echo suppression {db}dB",
  "回声消除已启用：以系统音频为参考消除麦克风中的面试官回声":
    "Echo cancellation is on: system audio is used as reference to remove interviewer echo from the microphone",

  // 问题识别
  问题识别: "Question detection",
  自动识别: "Auto",
  手动触发: "Manual",
  "已识别 {count} 个问题": "{count} questions detected",
  知识库参考: "Knowledge references",
  "{count} 条片段": "{count} snippets",
  手动识别当前问题: "Detect question now",
  用最近的面试官语音立即识别一次问题:
    "Detect a question immediately from recent interviewer speech",
  知识库中未召回到相关内容: "No relevant knowledge found",
  "（查询：{queries}）": " (queries: {queries})",
  当前问题: "Current question",
  置信度: "Confidence",
  "判定得分（阈值 0.5，越高越确定）": "Score (threshold 0.5, higher is more certain)",
  "正在监听面试官语音，识别到问题后会显示在这里。":
    "Listening for interviewer speech; detected questions appear here.",
  "开始转写后，面试官的问题会被自动识别；也可随时手动触发识别。":
    "Start transcription and interviewer questions are detected automatically, or trigger manually anytime.",

  // AI 回答
  "AI 回答": "AI answer",
  已完成: "Done",
  引用来源: "Sources",
  "{count} 篇": "{count} notes",
  "正在结合知识库生成回答…": "Generating an answer from your knowledge base…",
  "识别到问题后会自动生成回答，也可点击「重新生成」。":
    "Answers are generated when a question is detected; you can also regenerate.",
  "尚未识别到问题。开始转写并识别到问题后，这里会输出可直接口述的回答。":
    "No question detected yet. Once one is detected, a ready-to-speak answer appears here.",
  "在知识库中打开该笔记": "Open this note in the knowledge base",
  "该引用缺少笔记 ID": "This reference has no note ID",

  // 设置页
  "语音识别": "Speech recognition",
  语言: "Language",
  界面语言: "Interface language",

  // 一键演示
  "一键演示 · 完整链路": "One-click demo · full pipeline",
  "无需麦克风、模型或 API Key，用脚本化输入跑通真实链路":
    "No microphone, model or API key needed — scripted input drives the real pipeline",
  开始演示: "Start demo",
  "实时转写（面试官 / 我）": "Live transcript (interviewer / me)",
  "问题识别（规则打分 + 合段）": "Question detection (rules + merging)",
  "知识库召回（BM25 + 向量融合）": "Knowledge recall (BM25 + vector fusion)",
  "AI 流式回答 + 引用来源": "Streaming answer + sources",
  "演示会自动写入 4 篇示例笔记用于召回；结束后可在「会话历史」中回放与导出。":
    "The demo seeds 4 sample notes for recall; replay and export them later in Sessions.",
  "正在准备演示数据…": "Preparing demo data…",
  "未配置 AI Key，将使用离线拼装回答。":
    "No AI key configured — an offline composed answer will be used.",

  // 关于与更新
  关于与更新: "About & updates",
  "更新地址（latest.json）": "Update endpoint (latest.json)",
  "留空则关闭更新检查；填写后可由发布流水线生成的 latest.json 提供版本信息。":
    "Leave empty to disable update checks; a latest.json published by the release pipeline provides version info.",
  检查更新: "Check for updates",
  启动时自动检查: "Check automatically on startup",

  // 会话历史
  "共 {count} 场": "{count} sessions",
  刷新: "Refresh",
  删除: "Delete",
  "导出 Markdown": "Export Markdown",
  "导出 JSON": "Export JSON",
  转写时间线: "Transcript timeline",
  "转写时间线（{count} 条）": "Transcript timeline ({count} entries)",
  问答记录: "Q&A log",
  "问答记录（{count} 组）": "Q&A log ({count} pairs)",
  "该会话没有转写记录": "No transcript in this session",
  "该会话没有识别到问题": "No questions detected in this session",
  "（未生成回答）": "(no answer generated)",
  "选择左侧会话查看完整回放": "Select a session to replay",
  "正在加载会话详情…": "Loading session detail…",
  引用: "Sources",
  未命名笔记: "Untitled note",
  未结束: "Ongoing",
  "转写 {segments} 条 · 问答 {questions} 组":
    "{segments} transcript entries · {questions} Q&A pairs",
};

/** 读取本地保存的语言 */
export function loadLocale(): Locale {
  if (typeof window === "undefined") return DEFAULT_LOCALE;
  const stored = window.localStorage?.getItem(STORAGE_KEY);
  return LOCALES.some((item) => item.value === stored)
    ? (stored as Locale)
    : DEFAULT_LOCALE;
}

/** 保存语言选择 */
export function saveLocale(locale: Locale) {
  if (typeof window === "undefined") return;
  window.localStorage?.setItem(STORAGE_KEY, locale);
}

/**
 * 翻译：`params` 用于替换 `{name}` 占位符。
 * 英文缺失该项时回落中文原文（保证界面始终可读）。
 */
export function translate(
  locale: Locale,
  text: string,
  params?: Record<string, string | number>
): string {
  const template = locale === "en-US" ? (EN[text] ?? text) : text;
  if (!params) return template;
  return template.replace(/\{(\w+)\}/g, (match, key: string) =>
    params[key] === undefined ? match : String(params[key])
  );
}

/** 英文词典条目数（用于自检/测试） */
export function englishEntryCount(): number {
  return Object.keys(EN).length;
}
