import { useEffect, useRef, useState } from "react";
import { AlertCircle, Eraser, Mic, Play, Square, Volume2 } from "lucide-react";
import { cn } from "../../lib/utils";
import { useTranscriptEvents } from "../../hooks/useTranscriptEvents";
import { useInterviewStore, type CaptureSource } from "../../stores/interviewStore";

/** 说话人显示名 */
const SPEAKER_LABEL: Record<string, string> = {
  me: "我",
  interviewer: "面试官",
};

export default function TranscriptPanel() {
  const asrStatus = useInterviewStore((s) => s.asrStatus);
  const engines = useInterviewStore((s) => s.engines);
  const transcript = useInterviewStore((s) => s.transcript);
  const partial = useInterviewStore((s) => s.partial);
  const asrError = useInterviewStore((s) => s.asrError);
  const startingAsr = useInterviewStore((s) => s.startingAsr);
  const loadAsrStatus = useInterviewStore((s) => s.loadAsrStatus);
  const startTranscription = useInterviewStore((s) => s.startTranscription);
  const stopTranscription = useInterviewStore((s) => s.stopTranscription);
  const applyTranscriptPush = useInterviewStore((s) => s.applyTranscriptPush);
  const clearTranscript = useInterviewStore((s) => s.clearTranscript);

  const [source, setSource] = useState<CaptureSource>("system");
  const listRef = useRef<HTMLDivElement>(null);

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

  return (
    <section className="glass flex min-h-0 flex-col rounded-xl p-4">
      <header className="mb-3 flex items-center gap-2">
        <Mic className="h-4 w-4 text-primary-3" />
        <h2 className="text-[13px] font-semibold">实时转写</h2>
        <span
          className={cn(
            "rounded-full px-2 py-0.5 text-[10px]",
            running ? "bg-success/15 text-success" : "bg-white/[0.06] text-text-subtle"
          )}
        >
          {running ? "转写中" : "已停止"}
        </span>
        {asrStatus?.engine && (
          <span className="text-[10px] text-text-subtle">{asrStatus.engine}</span>
        )}
        <button
          title="清空当前转写显示"
          onClick={clearTranscript}
          className="ml-auto cursor-pointer rounded p-1 text-text-subtle transition-colors hover:bg-white/10 hover:text-text"
        >
          <Eraser className="h-3.5 w-3.5" />
        </button>
      </header>

      {/* 来源选择 */}
      <div className="mb-3 flex gap-1.5">
        {(
          [
            { key: "system" as const, label: "面试官（系统音频）", icon: Volume2 },
            { key: "mic" as const, label: "我（麦克风）", icon: Mic },
          ]
        ).map(({ key, label, icon: Icon }) => (
          <button
            key={key}
            disabled={running}
            onClick={() => setSource(key)}
            className={cn(
              "flex flex-1 cursor-pointer items-center justify-center gap-1.5 rounded-lg border px-2 py-1.5 text-[11px] transition-colors disabled:cursor-not-allowed disabled:opacity-60",
              source === key
                ? "border-primary/40 bg-primary/15 text-primary-3"
                : "border-white/10 bg-background-3/60 text-text-muted hover:text-text"
            )}
          >
            <Icon className="h-3 w-3" />
            {label}
          </button>
        ))}
      </div>

      {/* 引擎就绪提示 */}
      {engineBlocked && !asrError && (
        <p className="mb-3 rounded-lg border border-warning/30 bg-warning/10 px-3 py-2 text-[10px] leading-relaxed text-warning">
          {engine?.detail}
        </p>
      )}

      {asrError && (
        <p className="mb-3 flex items-start gap-1.5 rounded-lg border border-danger/30 bg-danger/10 px-3 py-2 text-[10px] leading-relaxed text-danger">
          <AlertCircle className="mt-0.5 h-3 w-3 shrink-0" />
          <span>{asrError}</span>
        </p>
      )}

      {/* 转写列表 */}
      <div
        ref={listRef}
        className="min-h-[140px] flex-1 overflow-y-auto rounded-lg border border-white/[0.06] bg-background-3/30 p-2"
      >
        {transcript.length === 0 && !partial ? (
          <p className="px-1 py-6 text-center text-[11px] leading-relaxed text-text-subtle">
            暂无转写内容。
            <br />
            开始转写后，面试官语音将实时显示在此处（音频不出本机）。
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
                  {SPEAKER_LABEL[line.speaker] ?? line.speaker}
                </span>
                <span className="text-[12px] leading-relaxed text-text">{line.text}</span>
              </li>
            ))}
            {partial && (
              <li key="partial" className="flex gap-2 opacity-70">
                <span className="mt-0.5 shrink-0 rounded bg-white/[0.06] px-1.5 py-0.5 text-[10px] text-text-subtle">
                  {SPEAKER_LABEL[partial.speaker] ?? partial.speaker}
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
        onClick={() => (running ? stopTranscription() : startTranscription(source, null))}
        disabled={startingAsr}
        className={cn(
          "mt-3 flex w-full cursor-pointer items-center justify-center gap-1.5 rounded-lg border px-4 py-2 text-[12px] transition-colors disabled:cursor-not-allowed disabled:opacity-50",
          running
            ? "border-danger/40 bg-danger/15 text-danger hover:bg-danger/25"
            : "border-primary/40 bg-primary/15 text-primary-3 hover:bg-primary/25"
        )}
      >
        {running ? <Square className="h-3.5 w-3.5" /> : <Play className="h-3.5 w-3.5" />}
        {running ? "停止转写" : "开始转写"}
      </button>
    </section>
  );
}
