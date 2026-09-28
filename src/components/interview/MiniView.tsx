import { useEffect, useMemo } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Maximize2, Power } from "lucide-react";
import { cn, isTauri } from "../../lib/utils";
import logoLockupLight from "../../assets/logo-lockup-light.png";
import logoLockupDark from "../../assets/logo-lockup-dark.png";
import { ipc } from "../../lib/ipc";
import { useAnswerEvents } from "../../hooks/useAnswerEvents";
import { useAnswerPipeline } from "../../hooks/useAnswerPipeline";
import { useInterviewShortcuts } from "../../hooks/useInterviewShortcuts";
import { useQuestionEvents } from "../../hooks/useQuestionEvents";
import { useTranscriptEvents } from "../../hooks/useTranscriptEvents";
import { useT } from "../../hooks/useT";
import { useAppStore } from "../../stores/appStore";
import { useInterviewStore } from "../../stores/interviewStore";
import type { QaPair } from "../../types";

/**
 * 迷你页视图（IA-13，就地迷你模式）：在主窗口内直接渲染，展示面试官实时转写 / 当前面试官问题与 AI 回答。
 * 由 `useAppStore.miniMode` 控制显隐；进入/退出时由后端调整主窗口尺寸与置顶，
 * 点击「恢复」即回到完整界面并还原窗口几何，因此不存在独立无边框窗口的「空白且无法关闭」问题。
 *
 * 链路能力与全屏一致（IA-13.6）：订阅转写/问题/回答三路事件，并经 `useAnswerPipeline`
 * 自动召回 + 自动生成回答，因此小窗内同样是逐字流式输出。
 */
export default function MiniView() {
  const asrStatus = useInterviewStore((s) => s.asrStatus);
  const currentQuestion = useInterviewStore((s) => s.currentQuestion);
  const transcript = useInterviewStore((s) => s.transcript);
  const partial = useInterviewStore((s) => s.partial);
  const answerText = useInterviewStore((s) => s.answerText);
  const answerStatus = useInterviewStore((s) => s.answerStatus);
  const answerError = useInterviewStore((s) => s.answerError);
  const sources = useInterviewStore((s) => s.sources);
  const persistedSources = useInterviewStore((s) => s.persistedSources);
  const applyQuestionPush = useInterviewStore((s) => s.applyQuestionPush);
  const applyAnswerPush = useInterviewStore((s) => s.applyAnswerPush);
  const applyTranscriptPush = useInterviewStore((s) => s.applyTranscriptPush);
  const loadAsrStatus = useInterviewStore((s) => s.loadAsrStatus);
  const hydrateFromPair = useInterviewStore((s) => s.hydrateFromPair);

  const exitMiniMode = useAppStore((s) => s.exitMiniMode);
  const t = useT();
  const sessionId = asrStatus?.sessionId ?? null;

  // 迷你页内同样响应快捷键与系统全局热键
  useInterviewShortcuts();
  // 三路事件订阅：转写（面试官实时文字）/ 问题（当前问题）/ 回答（流式 token）
  useTranscriptEvents(applyTranscriptPush);
  useQuestionEvents(applyQuestionPush);
  useAnswerEvents(applyAnswerPush);
  // 自动召回 + 自动生成回答（与全屏 AnswerPanel 共用同一份触发逻辑，IA-07 / IA-08 / IA-13）
  useAnswerPipeline();

  useEffect(() => {
    loadAsrStatus();
  }, [loadAsrStatus]);

  // 打开时恢复当前会话最近的问题与回答（迷你页不自造问答，重新生成仍由热键触发）
  useEffect(() => {
    if (!sessionId) return;
    ipc<QaPair | null>("latest_question", { sessionId })
      .then((pair) => {
        if (pair) hydrateFromPair(pair);
      })
      .catch((err) => console.error("恢复最近问答失败:", err));
  }, [sessionId, hydrateFromPair]);

  /**
   * 面试官一侧的实时文字：partial 优先（逐字增长），其次最近一条 final；
   * 与当前问题文本重复时不重复展示（「我」通道文字不进入迷你页）。
   */
  const live = useMemo(() => {
    if (partial?.speaker === "interviewer") {
      return { text: partial.text, provisional: true };
    }
    const lastFinal = [...transcript].reverse().find((line) => line.speaker === "interviewer");
    if (!lastFinal || lastFinal.text === currentQuestion?.text) return null;
    return { text: lastFinal.text, provisional: false };
  }, [transcript, partial, currentQuestion]);

  // 引用来源：优先实时召回结果，无则回退落库引用（与全屏 AnswerPanel 口径一致）
  const citations = sources.length ? sources : persistedSources;

  /** 退出应用：与标题栏「关闭」一致，遵循设置中的托盘行为配置 */
  const quitApp = () => {
    if (!isTauri()) return;
    getCurrentWindow()
      .close()
      .catch((err) => console.error("退出应用失败:", err));
  };

  const iconButton =
    "focus-ring flex h-6 w-7 cursor-pointer items-center justify-center rounded text-text-subtle transition-colors hover:bg-ghost/[0.06] hover:text-text";

  return (
    <div className="flex h-full w-full flex-col overflow-hidden rounded-xl border border-primary/25 bg-background text-text">
      <header
        data-tauri-drag-region
        className="flex h-8 shrink-0 items-center gap-2 border-b border-ghost/[0.06] bg-background-2/80 px-2.5"
      >
        <img
          src={logoLockupLight}
          alt=""
          draggable={false}
          className="h-4 w-auto select-none dark:hidden"
        />
        <img
          src={logoLockupDark}
          alt=""
          draggable={false}
          className="hidden h-4 w-auto select-none dark:block"
        />
        <div className="ml-auto flex items-center">
          <button className={iconButton} title={t("恢复完整界面")} onClick={exitMiniMode}>
            <Maximize2 className="h-3.5 w-3.5" />
          </button>
          <button
            className={cn(iconButton, "hover:bg-danger/80 hover:text-white")}
            title={t("退出应用")}
            onClick={quitApp}
          >
            <Power className="h-3.5 w-3.5" />
          </button>
        </div>
      </header>

      <div className="flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto p-2.5">
        {/* 面试官实时转写 / 当前问题 */}
        <div className="rounded-md border border-primary/25 bg-primary/[0.07] px-2.5 py-2">
          <p className="text-[10px] text-text-subtle">
            {currentQuestion ? t("当前问题") : t("面试官")}
          </p>
          {currentQuestion ? (
            <p className="mt-0.5 text-[12.5px] font-medium leading-relaxed">
              {currentQuestion.text}
            </p>
          ) : (
            <p
              className={cn(
                "mt-0.5 line-clamp-3 text-[12.5px] leading-relaxed",
                live ? "italic text-text-muted" : "text-text-subtle"
              )}
            >
              {live ? `${live.text}…` : "等待识别面试官问题…"}
            </p>
          )}
          {currentQuestion && live && (
            <p className="mt-1 line-clamp-1 border-t border-ghost/[0.06] pt-1 text-[10.5px] leading-relaxed text-text-subtle">
              {live.provisional ? `${live.text}…` : live.text}
            </p>
          )}
        </div>

        {/* AI 回答 */}
        <div className="flex min-h-0 flex-1 flex-col rounded-md border border-ghost/[0.06] bg-background-3/30 px-2.5 py-2">
          {answerError ? (
            <p className="text-[11.5px] leading-relaxed text-danger">{answerError}</p>
          ) : answerText ? (
            <p className="whitespace-pre-wrap text-[12.5px] leading-relaxed">
              {answerText}
              {answerStatus === "streaming" && (
                <span className="ml-0.5 inline-block h-3.5 w-1.5 animate-pulse bg-primary-3 align-middle" />
              )}
            </p>
          ) : (
            <p className="m-auto text-center text-[11px] leading-relaxed text-text-subtle">
              {answerStatus === "streaming" || answerStatus === "thinking"
                ? "正在生成回答…"
                : "识别到问题后这里会显示 AI 回答"}
            </p>
          )}
        </div>

        {citations.length > 0 && (
          <div className="flex flex-wrap items-center gap-1">
            <span className="text-[10px] text-text-subtle">引用：</span>
            {citations.slice(0, 3).map((hit) => (
              <span
                key={hit.noteId}
                className="max-w-[8rem] truncate rounded bg-ghost/[0.06] px-1.5 py-0.5 text-[10px] text-text-muted"
                title={hit.title}
              >
                {hit.title}
              </span>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
