import { useEffect } from "react";
import { useInterviewStore } from "../stores/interviewStore";

/** 面试中快捷键（应用内，窗口聚焦时生效） */
export const INTERVIEW_SHORTCUTS = [
  { keys: "Ctrl+Shift+Q", label: "一键提问" },
  { keys: "Ctrl+Shift+P", label: "暂停/继续采集" },
  { keys: "Ctrl+Shift+R", label: "重新生成回答" },
  { keys: "Ctrl+Shift+C", label: "复制回答" },
] as const;

/** 是否正在输入控件中（避免快捷键与输入冲突） */
function isTypingTarget(target: EventTarget | null): boolean {
  const element = target as HTMLElement | null;
  if (!element) return false;
  if (element.isContentEditable) return true;
  return ["INPUT", "TEXTAREA", "SELECT"].includes(element.tagName);
}

/**
 * 面试辅助快捷键：
 * - Ctrl/Cmd + Shift + Q 一键提问（手动触发问题识别）
 * - Ctrl/Cmd + Shift + P 暂停 / 继续采集
 * - Ctrl/Cmd + Shift + R 重新生成回答
 * - Ctrl/Cmd + Shift + C 复制回答
 */
export function useInterviewShortcuts() {
  const triggerQuestion = useInterviewStore((s) => s.triggerQuestion);
  const setAsrPaused = useInterviewStore((s) => s.setAsrPaused);
  const generateAnswer = useInterviewStore((s) => s.generateAnswer);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey) || !event.shiftKey) return;
      if (isTypingTarget(event.target)) return;

      const key = event.key.toLowerCase();
      const state = useInterviewStore.getState();
      const sessionId = state.asrStatus?.sessionId ?? null;

      switch (key) {
        case "q":
          event.preventDefault();
          triggerQuestion();
          break;
        case "p":
          event.preventDefault();
          setAsrPaused(!(state.asrStatus?.paused ?? false));
          break;
        case "r":
          event.preventDefault();
          if (sessionId && state.currentQaId) {
            generateAnswer(sessionId, state.currentQaId, true);
          }
          break;
        case "c":
          if (!state.answerText) return;
          event.preventDefault();
          navigator.clipboard?.writeText(state.answerText).catch((err) => {
            console.error("复制回答失败:", err);
          });
          break;
        default:
          break;
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [triggerQuestion, setAsrPaused, generateAnswer]);
}
