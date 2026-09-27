import { useEffect, useRef, useState } from "react";
import {
  AlertCircle,
  Eraser,
  Mic,
  Play,
  Square,
  Users,
  Volume2,
  Waves,
} from "lucide-react";
import { cn } from "../../lib/utils";
import { useTranscriptEvents } from "../../hooks/useTranscriptEvents";
import { useT } from "../../hooks/useT";
import { useInterviewStore, type CaptureSource } from "../../stores/interviewStore";

/** 采集模式：单路（mic/system）或双通道（dual） */
type CaptureMode = CaptureSource | "dual";

export default function TranscriptPanel() {
  const asrStatus = useInterviewStore((s) => s.asrStatus);
  const engines = useInterviewStore((s) => s.engines);
  const transcript = useInterviewStore((s) => s.transcript);
  const partial = useInterviewStore((s) => s.partial);
  const asrError = useInterviewStore((s) => s.asrError);
  const startingAsr = useInterviewStore((s) => s.startingAsr);
  const loadAsrStatus = useInterviewStore((s) => s.loadAsrStatus);
  const startTranscription = useInterviewStore((s) => s.startTranscription);
  const startDualTranscription = useInterviewStore((s) => s.startDualTranscription);
  const stopTranscription = useInterviewStore((s) => s.stopTranscription);
  const applyTranscriptPush = useInterviewStore((s) => s.applyTranscriptPush);
  const clearTranscript = useInterviewStore((s) => s.clearTranscript);

  const [mode, setMode] = useState<CaptureMode>("dual");
  const listRef = useRef<HTMLDivElement>(null);
  const t = useT();

  useTranscriptEvents(applyTranscriptPush);

  useEffect(() => {
    loadAsrStatus();
  }, [loadAsrStatus]);

  // 新内容自动滚动到底部
  useEffect(() => {
    const node = listRef.current;
    if (node) node.scrollTop = node.scrollHeight;
  }, [transcript.length, partial?.text]);

  const running = asrStatus?.running ?? false;
  const engine = engines[0];
  const engineBlocked = Boolean(engine && !engine.ready);

  const start = () => {
    if (mode === "dual") {
      startDualTranscription(null, null);
    } else {
      startTranscription(mode, null);
    }
  };

  const statusText = !running
    ? t("已停止")
    : asrStatus?.dual
      ? t("双通道转写中")
      : t("转写中");

  /** 说话人显示名（随语言切换） */
  const speakerLabel = (speaker: string) =>
    speaker === "interviewer" ? t("面试官") : t("我");

  return (
    <section className="glass flex min-h-0 flex-col rounded-xl p-4 shadow-card">
      <header className="mb-3 flex items-center gap-2">
        <Mic className="h-4 w-4 text-primary-3" />
        <h2 className="text-[13px] font-semibold">{t("实时转写")}</h2>
        <span
          className={cn(
            "rounded-full px-2 py-0.5 text-[10px]",
            running ? "bg-success/15 text-success" : "bg-white/[0.06] text-text-subtle"
          )}
        >
          {statusText}
        </span>
        {asrStatus?.engine && (
          <span className="text-[10px] text-text-subtle">{asrStatus.engine}</span>
        )}
        {asrStatus?.aecEnabled && (
          <span
            className="flex items-center gap-1 rounded-full bg-primary/15 px-2 py-0.5 text-[10px] text-primary-3"
            title={t("回声消除已启用：以系统音频为参考消除麦克风中的面试官回声")}
            >
            <Waves className="h-3 w-3" />
            {t("回声抑制 {db}dB", { db: (asrStatus.aecErleDb ?? 0).toFixed(1) })}
            </span>
            )}
            <button
            title={t("清空当前转写显示")}
          onClick={clearTranscript}
          className="focus-ring ml-auto cursor-pointer rounded p-1 text-text-subtle transition-colors hover:bg-white/[0.06] hover:text-text"
        >
          <Eraser className="h-3.5 w-3.5" />
        </button>
      </header>

      {/* 采集模式选择 */}
      <div className="mb-3 flex gap-1.5">
        {(
          [
            { key: "dual" as const, label: "双通道（区分说话人）", icon: Users },
            { key: "system" as const, label: "面试官（系统音频）", icon: Volume2 },
            { key: "mic" as const, label: "我（麦克风）", icon: Mic },
          ]
        ).map(({ key, label, icon: Icon }) => (
          <button
            key={key}
            disabled={running}
            onClick={() => setMode(key)}
            className={cn(
              "focus-ring flex flex-1 cursor-pointer items-center justify-center gap-1.5 rounded-lg border px-2 py-1.5 text-[11px] transition-colors disabled:cursor-not-allowed disabled:opacity-60",
              mode === key
                ? "border-primary/40 bg-primary/15 text-primary-3"
                : "border-white/10 bg-background-3/60 text-text-muted hover:text-text"
            )}
          >
            <Icon className="h-3 w-3" />
            {t(label)}
          </button>
        ))}
      </div>

      {/* 引擎就绪提示 */}
      {engineBlocked && !asrError && (
        <p className="mb-3 rounded-md border border-warning/30 bg-warning/10 px-3 py-2 text-[10px] leading-relaxed text-warning">
          {engine?.detail}
        </p>
      )}

      {asrError && (
        <p className="mb-3 flex items-start gap-1.5 rounded-md border border-danger/30 bg-danger/10 px-3 py-2 text-[10px] leading-relaxed text-danger">
          <AlertCircle className="mt-0.5 h-3 w-3 shrink-0" />
          <span>{asrError}</span>
        </p>
      )}

      {/* 转写列表 */}
      <div
        ref={listRef}
        className="min-h-[140px] flex-1 overflow-y-auto rounded-md border border-white/[0.06] bg-background-3/30 p-2"
      >
        {transcript.length === 0 && !partial ? (
          <p className="px-1 py-6 text-center text-[11px] leading-relaxed text-text-subtle">
            {t("暂无转写内容。")}
            <br />
            {t("双通道模式可同时转写「我」与「面试官」（音频不出本机）。")}
          </p>
        ) : (
          <ul className="flex flex-col gap-1.5">
            {transcript.map((line) => (
              <li key={line.id} className="flex gap-2">
                <span
                  className={cn(
                    "mt-0.5 shrink-0 rounded px-1.5 py-0.5 text-[10px]",
                    line.speaker === "interviewer"
                      ? "bg-primary/15 text-primary-3"
                      : "bg-white/[0.08] text-text-muted"
                  )}
                >
                  {speakerLabel(line.speaker)}
                </span>
                <span className="text-[12px] leading-relaxed text-text">{line.text}</span>
              </li>
            ))}
            {partial && (
              <li key="partial" className="flex gap-2 opacity-70">
                <span className="mt-0.5 shrink-0 rounded bg-white/[0.06] px-1.5 py-0.5 text-[10px] text-text-subtle">
                  {speakerLabel(partial.speaker)}
                </span>
                <span className="text-[12px] italic leading-relaxed text-text-muted">
                  {partial.text}…
                </span>
              </li>
            )}
          </ul>
        )}
      </div>

      <button
        onClick={() => (running ? stopTranscription() : start())}
        disabled={startingAsr}
        className={cn(
          "focus-ring mt-3 flex w-full cursor-pointer items-center justify-center gap-1.5 rounded-lg border px-4 py-2 text-[12px] transition-colors disabled:cursor-not-allowed disabled:opacity-50",
          running
            ? "border-danger/40 bg-danger/15 text-danger hover:bg-danger/25"
            : "border-primary/40 bg-primary/15 text-primary-3 hover:bg-primary/25"
        )}
      >
        {running ? <Square className="h-3.5 w-3.5" /> : <Play className="h-3.5 w-3.5" />}
        {running ? t("停止转写") : t("开始转写")}
      </button>
    </section>
  );
}
