import { Mic, Sparkles } from "lucide-react";
import AudioCapturePanel from "../components/interview/AudioCapturePanel";
import TranscriptPanel from "../components/interview/TranscriptPanel";
import { useInterviewStore } from "../stores/interviewStore";
import { cn } from "../lib/utils";

export default function InterviewPage() {
  const asrStatus = useInterviewStore((s) => s.asrStatus);
  const transcript = useInterviewStore((s) => s.transcript);

  const running = (asrStatus?.running ?? false) || transcript.length > 0;

  return (
    <div className="flex h-full flex-col">
      <div className="flex h-12 shrink-0 items-center gap-3 border-b border-white/5 px-4">
        <Mic className="h-4 w-4 text-primary-3" />
        <h1 className="text-[15px] font-semibold">面试辅助</h1>
        <span className="ml-auto flex items-center gap-1.5 rounded-full border border-white/8 bg-background-3/60 px-2.5 py-1 text-[11px] text-text-muted">
          <span
            className={cn(
              "h-1.5 w-1.5 rounded-full",
              running ? "animate-pulse-dot bg-success" : "bg-text-subtle"
            )}
          />
          {asrStatus?.running
            ? asrStatus.source === "system"
              ? "正在转写面试官"
              : "正在转写我的语音"
            : "未开始捕获"}
        </span>
      </div>

      <div className="flex min-h-0 flex-1 gap-4 overflow-hidden p-4">
        <div className="flex w-80 shrink-0 flex-col gap-3 overflow-y-auto">
          <AudioCapturePanel />
        </div>

        <div className="flex min-h-0 min-w-0 flex-1 flex-col gap-4">
          <TranscriptPanel />

          <div className="glass flex shrink-0 items-start gap-3 rounded-xl p-4">
            <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-gradient-to-br from-primary to-primary-3 shadow-lg shadow-primary/25">
              <Sparkles className="h-4 w-4 text-white" />
            </div>
            <div className="min-w-0">
              <h2 className="text-[13px] font-semibold">AI 回答</h2>
              <p className="mt-1 text-[12px] leading-relaxed text-text-muted">
                转写稳定后自动识别面试官问题，检索知识库并流式生成回答。
              </p>
              <span className="mt-2 inline-block rounded-md border border-white/8 bg-background-3/60 px-3 py-1.5 text-[11px] text-text-subtle">
                问题识别 / 检索 / AI 回答将在 IA-06 ~ IA-08 任务中实现
              </span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
