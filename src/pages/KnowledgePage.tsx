import { useEffect } from "react";
import {
  BookOpen,
  FilePlus2,
  FolderPlus,
  FolderTree,
  Loader2,
  Search,
} from "lucide-react";
import NoteList from "../components/knowledge/NoteList";
import NotePreview from "../components/knowledge/NotePreview";
import { useFileDrop } from "../hooks/useFileDrop";
import { pickDirectory, pickMarkdownFiles } from "../lib/dialog";
import { useKnowledgeStore } from "../stores/knowledgeStore";

const ACTION_BTN =
  "flex cursor-pointer items-center gap-1.5 rounded-lg border border-white/10 bg-background-3/60 px-3 py-1.5 text-[12px] text-text-muted transition-colors hover:border-primary/40 hover:text-text disabled:cursor-not-allowed disabled:opacity-50";

export default function KnowledgePage() {
  const notes = useKnowledgeStore((s) => s.notes);
  const groups = useKnowledgeStore((s) => s.groups);
  const loading = useKnowledgeStore((s) => s.loading);
  const importing = useKnowledgeStore((s) => s.importing);
  const selectedId = useKnowledgeStore((s) => s.selectedId);
  const keyword = useKnowledgeStore((s) => s.keyword);
  const error = useKnowledgeStore((s) => s.error);
  const load = useKnowledgeStore((s) => s.load);
  const importPaths = useKnowledgeStore((s) => s.importPaths);
  const select = useKnowledgeStore((s) => s.select);
  const setKeyword = useKnowledgeStore((s) => s.setKeyword);

  const { isDragging } = useFileDrop((paths) => {
    importPaths(paths);
  });

  useEffect(() => {
    load();
  }, [load]);

  const selected = notes.find((n) => n.id === selectedId) ?? null;

  const handleImportFiles = () => {
    pickMarkdownFiles().then((paths) => importPaths(paths));
  };

  const handleImportFolder = () => {
    pickDirectory().then((dir) => (dir ? importPaths([dir]) : 0));
  };

  return (
    <div className="relative flex h-full flex-col">
      {/* 页头 */}
      <div className="flex h-12 shrink-0 items-center gap-3 border-b border-white/5 px-4">
        <BookOpen className="h-4 w-4 text-primary-3" />
        <h1 className="text-[15px] font-semibold">知识库</h1>
        <span className="rounded-full bg-white/[0.06] px-2 py-0.5 text-[11px] text-text-muted">
          {notes.length} 篇
        </span>

        <div className="ml-auto flex items-center gap-2">
          <div className="flex w-60 items-center gap-2 rounded-lg border border-white/[0.08] bg-background-3/60 px-3 py-1.5">
            <Search className="h-3.5 w-3.5 shrink-0 text-text-subtle" />
            <input
              value={keyword}
              onChange={(e) => setKeyword(e.target.value)}
              placeholder="按标题 / 内容 / 标签筛选…"
              className="w-full border-0 bg-transparent p-0 text-[13px] text-text outline-none placeholder:text-text-subtle"
            />
          </div>
          <button
            onClick={handleImportFiles}
            disabled={importing}
            className={ACTION_BTN}
          >
            {importing ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <FilePlus2 className="h-3.5 w-3.5" />
            )}
            导入文件
          </button>
          <button
            onClick={handleImportFolder}
            disabled={importing}
            className={ACTION_BTN}
          >
            <FolderPlus className="h-3.5 w-3.5" />
            导入文件夹
          </button>
        </div>
      </div>

      {error && (
        <div className="border-b border-danger/20 bg-danger/10 px-4 py-1.5 text-[11px] text-danger">
          {error}
        </div>
      )}

      <div className="flex min-h-0 flex-1">
        {/* 笔记列表 */}
        <aside className="flex w-64 shrink-0 flex-col border-r border-white/5">
          <div className="flex items-center gap-2 px-3 py-2 text-[11px] text-text-muted">
            <FolderTree className="h-3.5 w-3.5" />
            全部笔记
            <span className="ml-auto text-text-subtle">{groups.length} 个群组</span>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto px-2 pb-3">
            {loading ? (
              <p className="px-2 py-6 text-center text-[12px] text-text-subtle">
                正在加载…
              </p>
            ) : (
              <NoteList
                notes={notes}
                selectedId={selectedId}
                keyword={keyword}
                onSelect={select}
              />
            )}
          </div>
        </aside>

        {/* 内容预览 */}
        <section className="min-w-0 flex-1">
          <NotePreview note={selected} />
        </section>
      </div>

      {/* 拖拽提示 */}
      {isDragging && (
        <div className="pointer-events-none absolute inset-0 z-20 flex items-center justify-center bg-background/80 backdrop-blur-sm">
          <div className="flex flex-col items-center gap-2 rounded-xl border-2 border-dashed border-primary/50 bg-background-2/60 px-14 py-10">
            <FilePlus2 className="h-8 w-8 text-primary-3" />
            <p className="text-[13px] text-text">释放以导入 Markdown 文件</p>
            <p className="text-[11px] text-text-subtle">
              支持单文件与整个文件夹
            </p>
          </div>
        </div>
      )}
    </div>
  );
}
