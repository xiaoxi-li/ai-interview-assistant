import { useCallback, useEffect, useRef } from "react";
import { AlertCircle, Check, X } from "lucide-react";
import { cn } from "../../lib/utils";
import {
  useSettingsStore,
  type SettingsFeedbackTarget,
  type SettingsSaveFeedback,
} from "../../stores/settingsStore";

/** 成功提示自动淡出时长（毫秒）；失败提示常驻，直到手动关闭或下一次操作 */
export const FEEDBACK_DISMISS_MS = 3000;

interface SaveFeedbackViewProps {
  /** 当前反馈；null 表示无提示（不占位） */
  feedback: SettingsSaveFeedback | null;
  /** 关闭提示（也用于成功提示自动淡出） */
  onDismiss: () => void;
}

/**
 * 受控形态的保存反馈：供不经过 `settingsStore` 的场景（如 `UpdateSection`）复用。
 * 成功约 3 秒后自动淡出，失败保留至手动关闭或下一次操作覆盖。
 */
export function SaveFeedbackView({ feedback, onDismiss }: SaveFeedbackViewProps) {
  const dismissRef = useRef(onDismiss);
  const success = feedback?.kind === "success";

  useEffect(() => {
    dismissRef.current = onDismiss;
  }, [onDismiss]);

  useEffect(() => {
    if (!success) return;
    const timer = window.setTimeout(() => dismissRef.current(), FEEDBACK_DISMISS_MS);
    return () => window.clearTimeout(timer);
  }, [feedback, success]);

  if (!feedback) return null;

  return (
    <span
      role={success ? "status" : "alert"}
      aria-live={success ? "polite" : "assertive"}
      className={cn(
        "animate-fade-in flex min-w-0 items-center gap-1.5 rounded-md border px-2 py-1 text-[11px] leading-relaxed",
        success
          ? "border-success/30 bg-success/10 text-success"
          : "border-danger/30 bg-danger/10 text-danger"
      )}
    >
      {success ? (
        <Check className="h-3 w-3 shrink-0" />
      ) : (
        <AlertCircle className="h-3 w-3 shrink-0" />
      )}
      <span className="min-w-0 truncate" title={feedback.text}>
        {feedback.text}
      </span>
      {!success && (
        <button
          type="button"
          title="关闭提示"
          aria-label="关闭提示"
          onClick={onDismiss}
          className="focus-ring -mr-0.5 shrink-0 cursor-pointer rounded opacity-70 transition-colors hover:opacity-100"
        >
          <X className="h-3 w-3" />
        </button>
      )}
    </span>
  );
}

/**
 * store 连接形态：各设置分区在动作行渲染 `<SaveFeedback target="ai" />` 即可。
 * 成功后由本组件定时清除；离开设置页时丢弃成功提示，避免返回后残留。
 */
export function SaveFeedback({ target }: { target: SettingsFeedbackTarget }) {
  const feedback = useSettingsStore((state) => state.saveFeedback[target]);
  const clearSaveFeedback = useSettingsStore((state) => state.clearSaveFeedback);

  const dismiss = useCallback(
    () => clearSaveFeedback(target),
    [clearSaveFeedback, target]
  );

  useEffect(
    () => () => {
      const current = useSettingsStore.getState().saveFeedback[target];
      if (current?.kind === "success") clearSaveFeedback(target);
    },
    [clearSaveFeedback, target]
  );

  return <SaveFeedbackView feedback={feedback} onDismiss={dismiss} />;
}
