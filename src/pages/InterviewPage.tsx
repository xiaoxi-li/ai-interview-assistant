import { Mic, Sparkles } from "lucide-react";
import AudioCapturePanel from "../components/interview/AudioCapturePanel";

export default function InterviewPage() {
  return (
    <div className="flex h-full flex-col">
      <div className="flex h-12 shrink-0 items-center gap-3 border-b border-white/5 px-4">
        <Mic className="h-4 w-4 text-primary-3" />
        <h1 className="text-[15px] font-semibold">面试辅助</h1>
        <span className="ml-auto flex items-center gap-1.5 rounded-full border border-white/8 bg-background-3/60 px-2.5 py-1 text-[11px] text-text-muted">
          <span className="h-1.5 w-1.5 rounded-full bg-text-subtle" />
          未开始捕获
        </span>
      </div>

      <div className="flex min-h-0 flex-1 gap-4 overflow-hidden p-4">
        <div className="w-80 shrink-0 overflow-y-auto">
          <AudioCapturePanel />
        </div>

        <div className="flex min-w-0 flex-1 items-center justify-center">
        <div className="glass flex max-w-lg flex-col items-center gap-3 rounded-xl px-10 py-12 text-center">
          <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-gradient-to-br from-primary to-primary-3 shadow-lg shadow-primary/25">
            <Sparkles className="h-6 w-6 text-white" />
          </div>
          <h2 className="text-[15px] font-semibold">实时面试辅助</h2>
          <p className="text-[12px] leading-relaxed text-text-muted">
            开始捕获后，面试官语音将实时转写为文字，自动识别问题并检索
            知识库，结合 AI 流式生成回答。
          </p>
          <div className="mt-1 rounded-md border border-white/8 bg-background-3/60 px-3 py-1.5 text-[11px] text-text-subtle">
            转写与 AI 回答链路将在 IA-04 / IA-08 任务中实现
          </div>
        </div>
        </div>
      </div>
    </div>
  );
}
