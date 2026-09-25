import { BookOpen, FolderTree, Search } from "lucide-react";

export default function KnowledgePage() {
  return (
    <div className="flex h-full flex-col">
      <div className="flex h-12 shrink-0 items-center gap-3 border-b border-white/5 px-4">
        <BookOpen className="h-4 w-4 text-primary-3" />
        <h1 className="text-[15px] font-semibold">知识库</h1>
        <div className="ml-auto flex w-72 items-center gap-2 rounded-lg border border-white/8 bg-background-3/60 px-3 py-1.5">
          <Search className="h-3.5 w-3.5 text-text-subtle" />
          <input
            className="w-full border-0 bg-transparent p-0 text-[13px] text-text outline-none placeholder:text-text-subtle"
            placeholder="全文 / 语义检索笔记…"
          />
        </div>
      </div>

      <div className="flex min-h-0 flex-1">
        <aside className="w-56 shrink-0 border-r border-white/5 p-3">
          <div className="mb-2 flex items-center gap-2 px-1 text-[12px] text-text-muted">
            <FolderTree className="h-3.5 w-3.5" />
            群组
          </div>
          <p className="px-1 text-[12px] leading-relaxed text-text-subtle">
            多级群组树将在 KB-04 任务中实现
          </p>
        </aside>

        <section className="flex min-w-0 flex-1 items-center justify-center">
          <div className="glass flex max-w-md flex-col items-center gap-3 rounded-xl px-10 py-12 text-center">
            <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-gradient-to-br from-primary to-primary-3 shadow-lg shadow-primary/25">
              <BookOpen className="h-6 w-6 text-white" />
            </div>
            <h2 className="text-[15px] font-semibold">知识库尚空</h2>
            <p className="text-[12px] leading-relaxed text-text-muted">
              导入 Markdown 笔记后，可在此进行源码/预览双视图编辑、
              中文全文检索与向量语义检索。
            </p>
            <div className="mt-1 rounded-md border border-white/8 bg-background-3/60 px-3 py-1.5 text-[11px] text-text-subtle">
              导入与编辑功能将在 KB-01 / KB-02 任务中实现
            </div>
          </div>
        </section>
      </div>
    </div>
  );
}
