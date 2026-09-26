import { Mic } from "lucide-react";
import AudioCapturePanel from "../components/interview/AudioCapturePanel";
import TranscriptPanel from "../components/interview/TranscriptPanel";
import QuestionPanel from "../components/interview/QuestionPanel";
import AnswerPanel from "../components/interview/AnswerPanel";
import ShortcutBar from "../components/interview/ShortcutBar";
import { useT } from "../hooks/useT";
import { useInterviewStore } from "../stores/interviewStore";
import { cn } from "../lib/utils";

export default function InterviewPage() {
  const asrStatus = useInterviewStore((s) => s.asrStatus);
  const t = useT();

  const running = asrStatus?.running ?? false;
  const paused = asrStatus?.paused ?? false;

  return (
    <div className="flex h-full flex-col">
      <div className="flex h-12 shrink-0 items-center gap-3 border-b border-white/5 px-4">
        <Mic className="h-4 w-4 text-primary-3" />
        <h1 className="text-[15px] font-semibold">{t("面试辅助")}</h1>
        <span className="ml-auto flex items-center gap-1.5 rounded-full border border-white/8 bg-background-3/60 px-2.5 py-1 text-[11px] text-text-muted">
          <span
            className={cn(
              "h-1.5 w-1.5 rounded-full",
              running ? "animate-pulse-dot bg-success" : "bg-text-subtle"
            )}
          />
          {!asrStatus?.running
            ? t("未开始捕获")
            : paused
              ? t("已暂停采集")
              : asrStatus.source === "system"
                ? t("正在转写面试官")
                : t("正在转写我的语音")}
        </span>
      </div>

      <div className="flex min-h-0 flex-1 gap-4 overflow-hidden p-4">
        <div className="flex w-80 shrink-0 flex-col gap-3 overflow-y-auto">
          <AudioCapturePanel />
        </div>

        <div className="flex min-h-0 min-w-0 flex-1 flex-col gap-4 overflow-y-auto">
          <ShortcutBar />
          <TranscriptPanel />
          <QuestionPanel />
          <AnswerPanel />
        </div>
      </div>
    </div>
  );
}
