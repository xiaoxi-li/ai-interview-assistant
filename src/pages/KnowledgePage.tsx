import { useEffect, useMemo, useState } from "react";
import {
  BookOpen,
  FilePlus2,
  FileText,
  FolderPlus,
  Loader2,
  Plus,
  Search,
  Trash2,
} from "lucide-react";
import GroupTree from "../components/knowledge/GroupTree";
import NoteList from "../components/knowledge/NoteList";
import MarkdownEditor from "../components/knowledge/MarkdownEditor";
import { useFileDrop } from "../hooks/useFileDrop";
import { useNoteDraft } from "../hooks/useNoteDraft";
import { pickDirectory, pickMarkdownFiles } from "../lib/dialog";
import { countNotesByGroup, filterNotesByGroup } from "../lib/groupTree";
import { useKnowledgeStore } from "../stores/knowledgeStore";
import { cn } from "../lib/utils";
import type { ViewMode } from "../types";

const ACTION_BTN =
  "flex cursor-pointer items-center gap-1.5 rounded-lg border border-white/10 bg-background-3/60 px-3 py-1.5 text-[12px] text-text-muted transition-colors hover:border-primary/40 hover:text-text disabled:cursor-not-allowed disabled:opacity-50";
const PRIMARY_BTN =
  "flex cursor-pointer items-center gap-1.5 rounded-lg border border-primary/40 bg-primary/15 px-3 py-1.5 text-[12px] text-primary-3 transition-colors hover:bg-primary/25";

export default function KnowledgePage() {
  const notes = useKnowledgeStore((s) => s.notes);
  const trashNotes = useKnowledgeStore((s) => s.trashNotes);
  const groups = useKnowledgeStore((s) => s.groups);
  const view = useKnowledgeStore((s) => s.view);
  const selectedGroupId = useKnowledgeStore((s) => s.selectedGroupId);
  const loading = useKnowledgeStore((s) => s.loading);
  const importing = useKnowledgeStore((s) => s.importing);
  const saving = useKnowledgeStore((s) => s.saving);
  const savedAt = useKnowledgeStore((s) => s.savedAt);
  const selectedId = useKnowledgeStore((s) => s.selectedId);
  const keyword = useKnowledgeStore((s) => s.keyword);
  const error = useKnowledgeStore((s) => s.error);

  const load = useKnowledgeStore((s) => s.load);
  const importPaths = useKnowledgeStore((s) => s.importPaths);
  const createNote = useKnowledgeStore((s) => s.createNote);
  const trashNote = useKnowledgeStore((s) => s.trashNote);
  const restoreNote = useKnowledgeStore((s) => s.restoreNote);
  const purgeNote = useKnowledgeStore((s) => s.purgeNote);
  const emptyTrash = useKnowledgeStore((s) => s.emptyTrash);
  const moveNote = useKnowledgeStore((s) => s.moveNote);
  const createGroup = useKnowledgeStore((s) => s.createGroup);
  const renameGroup = useKnowledgeStore((s) => s.renameGroup);
  const deleteGroup = useKnowledgeStore((s) => s.deleteGroup);
  const moveGroup = useKnowledgeStore((s) => s.moveGroup);
  const reorderGroups = useKnowledgeStore((s) => s.reorderGroups);
  const setView = useKnowledgeStore((s) => s.setView);
  const setSelectedGroup = useKnowledgeStore((s) => s.setSelectedGroup);
  const select = useKnowledgeStore((s) => s.select);
  const setKeyword = useKnowledgeStore((s) => s.setKeyword);

  const [viewMode, setViewMode] = useState<ViewMode>("split");

  const { isDragging } = useFileDrop((paths) => {
    importPaths(paths);
  });

  useEffect(() => {
    load();
  }, [load]);

  const isTrash = view === "trash";
  const groupScopedNotes = useMemo(
    () => filterNotesByGroup(notes, groups, selectedGroupId),
    [notes, groups, selectedGroupId]
  );
  const noteCounts = useMemo(() => countNotesByGroup(notes), [notes]);
  const ungroupedCount = useMemo(
    () => notes.filter((n) => n.groupId === null).length,
    [notes]
  );

  const selected = notes.find((n) => n.id === selectedId) ?? null;
  const draft = useNoteDraft(selected);
  const visibleNotes = isTrash ? trashNotes : groupScopedNotes;

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
          <div className="flex w-56 items-center gap-2 rounded-lg border border-white/[0.08] bg-background-3/60 px-3 py-1.5">
            <Search className="h-3.5 w-3.5 shrink-0 text-text-subtle" />
            <input
              value={keyword}
              onChange={(e) => setKeyword(e.target.value)}
              placeholder="按标题 / 内容 / 标签筛选…"
              className="w-full border-0 bg-transparent p-0 text-[13px] text-text outline-none placeholder:text-text-subtle"
            />
          </div>
          <button onClick={() => createNote()} className={PRIMARY_BTN}>
            <Plus className="h-3.5 w-3.5" />
            新建笔记
          </button>
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
        {/* 侧栏 */}
        <aside className="flex w-64 shrink-0 flex-col border-r border-white/5">
          <div className="flex items-center gap-1 px-2 pt-2">
            <button
              onClick={() => setView("all")}
              className={cn(
                "flex flex-1 cursor-pointer items-center justify-center gap-1.5 rounded-md py-1.5 text-[11px] transition-colors",
                !isTrash
                  ? "bg-primary/15 text-primary-3"
                  : "text-text-subtle hover:text-text-muted"
              )}
            >
              <BookOpen className="h-3.5 w-3.5" />
              全部 {notes.length}
            </button>
            <button
              onClick={() => setView("trash")}
              className={cn(
                "flex flex-1 cursor-pointer items-center justify-center gap-1.5 rounded-md py-1.5 text-[11px] transition-colors",
                isTrash
                  ? "bg-danger/15 text-danger"
                  : "text-text-subtle hover:text-text-muted"
              )}
            >
              <Trash2 className="h-3.5 w-3.5" />
              回收站 {trashNotes.length}
            </button>
          </div>

          {isTrash && trashNotes.length > 0 && (
            <button
              onClick={emptyTrash}
              className="mx-2 mt-2 cursor-pointer rounded-md border border-danger/30 bg-danger/10 py-1 text-[11px] text-danger transition-colors hover:bg-danger/20"
            >
              清空回收站
            </button>
          )}

          <div className="min-h-0 flex-1 overflow-y-auto px-2 pb-3 pt-2">
            {!isTrash && (
              <GroupTree
                groups={groups}
                selectedGroupId={selectedGroupId}
                noteCounts={noteCounts}
                totalCount={notes.length}
                ungroupedCount={ungroupedCount}
                onSelect={setSelectedGroup}
                onCreate={createGroup}
                onRename={renameGroup}
                onDelete={deleteGroup}
                onMove={moveGroup}
                onReorder={reorderGroups}
                onDropNote={moveNote}
              />
            )}

            <div className="mt-3 flex items-center gap-2 px-1 pb-1 text-[10px] uppercase tracking-wide text-text-subtle">
              {isTrash ? "回收站" : "笔记"}
            </div>

            {loading ? (
              <p className="px-2 py-6 text-center text-[12px] text-text-subtle">
                正在加载…
              </p>
            ) : (
              <NoteList
                notes={visibleNotes}
                selectedId={selectedId}
                keyword={keyword}
                mode={view}
                onSelect={select}
                onTrash={trashNote}
                onRestore={restoreNote}
                onPurge={purgeNote}
              />
            )}
          </div>
        </aside>

        {/* 主区 */}
        <section className="min-w-0 flex-1">
          {isTrash ? (
            <div className="flex h-full items-center justify-center">
              <div className="glass flex max-w-md flex-col items-center gap-3 rounded-xl px-10 py-12 text-center">
                <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-danger/15">
                  <Trash2 className="h-6 w-6 text-danger" />
                </div>
                <h2 className="text-[15px] font-semibold">回收站</h2>
                <p className="text-[12px] leading-relaxed text-text-muted">
                  已删除的笔记不会出现在列表中。可在此恢复，或彻底删除后不可找回。
                </p>
                <p className="text-[11px] text-text-subtle">共 {trashNotes.length} 篇</p>
              </div>
            </div>
          ) : selected ? (
            <MarkdownEditor
              title={draft.title}
              content={draft.content}
              viewMode={viewMode}
              saving={saving}
              savedAt={savedAt}
              onTitleChange={draft.setTitle}
              onContentChange={draft.setContent}
              onViewModeChange={setViewMode}
            />
          ) : (
            <div className="flex h-full items-center justify-center">
              <div className="glass flex max-w-md flex-col items-center gap-3 rounded-xl px-10 py-12 text-center">
                <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-gradient-to-br from-primary to-primary-3 shadow-lg shadow-primary/25">
                  <FileText className="h-6 w-6 text-white" />
                </div>
                <h2 className="text-[15px] font-semibold">知识库尚空</h2>
                <p className="text-[12px] leading-relaxed text-text-muted">
                  新建笔记或导入 Markdown 文件，即可进行源码 / 预览双视图编辑。
                </p>
              </div>
            </div>
          )}
        </section>
      </div>

      {/* 拖拽导入提示 */}
      {isDragging && (
        <div className="pointer-events-none absolute inset-0 z-20 flex items-center justify-center bg-background/80 backdrop-blur-sm">
          <div className="flex flex-col items-center gap-2 rounded-xl border-2 border-dashed border-primary/50 bg-background-2/60 px-14 py-10">
            <FilePlus2 className="h-8 w-8 text-primary-3" />
            <p className="text-[13px] text-text">释放以导入 Markdown 文件</p>
            <p className="text-[11px] text-text-subtle">支持单文件与整个文件夹</p>
          </div>
        </div>
      )}
    </div>
  );
}
