import { History } from "lucide-react";

export default function SessionsPage() {
  return (
    <div className="flex h-full flex-col">
      <div className="flex h-12 shrink-0 items-center gap-3 border-b border-white/5 px-4">
        <History className="h-4 w-4 text-primary-3" />
        <h1 className="text-[15px] font-semibold">会话历史</h1>
      </div>

      <div className="flex min-h-0 flex-1 items-center justify-center">
        <div className="glass flex max-w-lg flex-col items-center gap-3 rounded-xl px-10 py-12 text-center">
          <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-gradient-to-br from-primary to-primary-3 shadow-lg shadow-primary/25">
            <History className="h-6 w-6 text-white" />
          </div>
          <h2 className="text-[15px] font-semibold">暂无面试会话</h2>
          <p className="text-[12px] leading-relaxed text-text-muted">
            面试会话将记录完整转写、问答与时间线，支持回放与
            Markdown / JSON 导出。
          </p>
          <div className="mt-1 rounded-md border border-white/8 bg-background-3/60 px-3 py-1.5 text-[11px] text-text-subtle">
            会话回放与导出将在 IA-10 任务中实现
          </div>
        </div>
      </div>
    </div>
  );
}
