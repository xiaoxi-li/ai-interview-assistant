import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { isTauri } from "../lib/utils";
import { useAppStore } from "../stores/appStore";
import { useInterviewStore } from "../stores/interviewStore";

/** 面试中可执行的动作（与后端 HOTKEY_ACTIONS 一致） */
export type InterviewAction =
  | "ask"
  | "regenerate"
  | "copy_answer"
  | "pause_capture"
  | "toggle_mini";

/** 面试中快捷键（应用内，窗口聚焦时生效；全局热键由 IA-11 在系统层注册） */
export const INTERVIEW_SHORTCUTS = [
  { key: "a", keys: "Ctrl+Shift+A", label: "一键提问" },
  { key: "p", keys: "Ctrl+Shift+P", label: "暂停/继续采集" },
  { key: "r", keys: "Ctrl+Shift+R", label: "重新生成回答" },
  { key: "c", keys: "Ctrl+Shift+C", label: "复制回答" },
  { key: "m", keys: "Ctrl+Shift+M", label: "迷你模式" },
] as const;

/** 是否正在输入控件中（避免快捷键与输入冲突） */
function isTypingTarget(target: EventTarget | null): boolean {
  const element = target as HTMLElement | null;
  if (!element) return false;
  if (element.isContentEditable) return true;
  return ["INPUT", "TEXTAREA", "SELECT"].includes(element.tagName);
}

/** 切换迷你模式（IA-13：应用内就地切换，与标题栏按钮、托盘同一入口） */
function toggleMiniMode() {
  useAppStore.getState().toggleMiniMode();
}

/** 执行面试动作：应用内快捷键与系统全局热键共用同一入口 */
export function runInterviewAction(action: InterviewAction) {
  const state = useInterviewStore.getState();
  const sessionId = state.asrStatus?.sessionId ?? null;

  switch (action) {
    case "ask":
      state.triggerQuestion();
      break;
    case "pause_capture":
      state.setAsrPaused(!(state.asrStatus?.paused ?? false));
      break;
    case "regenerate":
      if (sessionId && state.currentQaId) {
        state.generateAnswer(sessionId, state.currentQaId, true);
      }
      break;
    case "copy_answer":
      if (state.answerText) {
        navigator.clipboard?.writeText(state.answerText).catch((err) => {
          console.error("复制回答失败:", err);
        });
      }
      break;
    case "toggle_mini":
      toggleMiniMode();
      break;
    default:
      break;
  }
}

/**
 * 面试辅助快捷键：
 * - 应用内：Ctrl/Cmd + Shift + A/P/R/C/M
 * - 系统级：由 Rust 侧注册，触发后广播 `shortcut://action`
 */
export function useInterviewShortcuts() {
  // 应用内键盘快捷键
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey) || !event.shiftKey) return;
      if (isTypingTarget(event.target)) return;

      const key = event.key.toLowerCase();
      const matched = INTERVIEW_SHORTCUTS.find((item) => item.key === key);
      if (!matched) return;

      event.preventDefault();
      runInterviewAction(actionForKey(matched.key));
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  // 系统全局热键事件
  useEffect(() => {
    if (!isTauri()) return;

    const unlisten = listen<{ action: InterviewAction }>("shortcut://action", (event) => {
      runInterviewAction(event.payload.action);
    });

    return () => {
      unlisten
        .then((fn) => fn())
        .catch((err) => console.error("取消全局快捷键监听失败:", err));
    };
  }, []);
}

/** 按键 → 动作 */
function actionForKey(key: string): InterviewAction {
  switch (key) {
    case "a":
      return "ask";
    case "p":
      return "pause_capture";
    case "r":
      return "regenerate";
    case "c":
      return "copy_answer";
    default:
      return "toggle_mini";
  }
}
