import { useEffect, useState } from "react";
import { Check, Copy, Eye, EyeOff, Pause, Pin, Play, RefreshCw, Zap } from "lucide-react";
import { cn } from "../../lib/utils";
import {
  INTERVIEW_SHORTCUTS,
  useInterviewShortcuts,
} from "../../hooks/useInterviewShortcuts";
import { useT } from "../../hooks/useT";
import { useInterviewStore } from "../../stores/interviewStore";
import { useSettingsStore } from "../../stores/settingsStore";

/** 面试中快捷操作条（IA-12）：一键提问 / 暂停 / 重新生成 / 复制 / 隐身模式（含键盘快捷键） */
export default function ShortcutBar() {
  // 快捷操作条常驻面试页，快捷键注册在此处，避免多处重复绑定
  useInterviewShortcuts();

  const t = useT();

  const asrStatus = useInterviewStore((s) => s.asrStatus);
  const currentQaId = useInterviewStore((s) => s.currentQaId);
  const answerText = useInterviewStore((s) => s.answerText);
  const streaming = useInterviewStore((s) => s.answerStatus === "streaming");
  const triggerQuestion = useInterviewStore((s) => s.triggerQuestion);
  const setAsrPaused = useInterviewStore((s) => s.setAsrPaused);
  const generateAnswer = useInterviewStore((s) => s.generateAnswer);

  const interfaceConfig = useSettingsStore((s) => s.interfaceConfig);
  const loadInterfaceSettings = useSettingsStore((s) => s.loadInterfaceSettings);
  const setStealthWindow = useSettingsStore((s) => s.setStealthWindow);

  const [copied, setCopied] = useState(false);

  const sessionId = asrStatus?.sessionId ?? null;
  const running = asrStatus?.running ?? false;
  const paused = asrStatus?.paused ?? false;
  const alwaysOnTop = interfaceConfig?.alwaysOnTop ?? false;
  const hideFromCapture = interfaceConfig?.hideFromCapture ?? false;

  useEffect(() => {
    if (!interfaceConfig) loadInterfaceSettings();
  }, [interfaceConfig, loadInterfaceSettings]);

  const shortcut = (label: string) =>
    INTERVIEW_SHORTCUTS.find((item) => item.label.includes(label))?.keys ?? "";

  const handleCopy = () => {
    if (!answerText) return;
    navigator.clipboard
      ?.writeText(answerText)
      .then(() => {
        setCopied(true);
        window.setTimeout(() => setCopied(false), 1500);
      })
      .catch((err) => console.error("复制回答失败:", err));
  };

  const buttonClass =
    "flex cursor-pointer items-center gap-1.5 rounded-lg border border-white/10 bg-background-3/60 px-2.5 py-1.5 text-[11px] text-text-muted transition-colors hover:border-primary/30 hover:text-primary-3 disabled:cursor-not-allowed disabled:opacity-40";

  return (
    <div className="glass flex flex-wrap items-center gap-2 rounded-xl px-3 py-2">
      <span className="flex items-center gap-1.5 text-[11px] font-medium text-text-muted">
        <Zap className="h-3.5 w-3.5 text-primary-3" />
        {t("快捷操作")}
      </span>

      <button
        className={buttonClass}
        disabled={!running}
        title={t("一键提问（{keys}）", { keys: shortcut("一键提问") })}
        onClick={() => triggerQuestion()}
      >
        <Zap className="h-3 w-3" />
        {t("一键提问")}
      </button>

      <button
        className={cn(buttonClass, paused && "border-warning/40 text-warning")}
        disabled={!running}
        title={
          paused
            ? t("继续采集（{keys}）", { keys: shortcut("暂停") })
            : t("暂停采集（{keys}）", { keys: shortcut("暂停") })
        }
        onClick={() => setAsrPaused(!paused)}
      >
        {paused ? <Play className="h-3 w-3" /> : <Pause className="h-3 w-3" />}
        {paused ? t("继续采集") : t("暂停采集")}
      </button>

      <button
        className={buttonClass}
        disabled={!currentQaId || streaming}
        title={t("重新生成回答（{keys}）", { keys: shortcut("重新生成") })}
        onClick={() => sessionId && generateAnswer(sessionId, currentQaId, true)}
      >
        <RefreshCw className="h-3 w-3" />
        {t("重新生成")}
      </button>

      <button
        className={buttonClass}
        disabled={!answerText}
        title={t("复制回答（{keys}）", { keys: shortcut("复制") })}
        onClick={handleCopy}
      >
        {copied ? <Check className="h-3 w-3 text-success" /> : <Copy className="h-3 w-3" />}
        {t("复制回答")}
      </button>

      {/* 隐身模式（IA-11） */}
      <span className="mx-1 h-4 w-px bg-white/10" />
      <button
        className={cn(buttonClass, alwaysOnTop && "border-primary/40 text-primary-3")}
        title={t("隐身模式：窗口置顶")}
        aria-pressed={alwaysOnTop}
        onClick={() => setStealthWindow({ alwaysOnTop: !alwaysOnTop })}
      >
        <Pin className="h-3 w-3" />
        {t("置顶")}
      </button>
      <button
        className={cn(buttonClass, hideFromCapture && "border-primary/40 text-primary-3")}
        title={t("隐身模式：屏幕共享时隐藏窗口内容（防录屏）")}
        aria-pressed={hideFromCapture}
        onClick={() => setStealthWindow({ hideFromCapture: !hideFromCapture })}
      >
        {hideFromCapture ? <EyeOff className="h-3 w-3" /> : <Eye className="h-3 w-3" />}
        {t("防录屏")}
      </button>

      <span className="ml-auto hidden text-[10px] text-text-subtle lg:inline">
        {INTERVIEW_SHORTCUTS.map((item) => item.keys).join(" / ")}
      </span>
    </div>
  );
}
