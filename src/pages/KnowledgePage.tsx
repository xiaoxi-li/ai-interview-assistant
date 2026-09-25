import { useEffect, useMemo, useState } from "react";
import {
  BookOpen,
  CircleCheck,
  Database,
  FileDown,
  FileJson,
  FilePlus2,
  FileText,
  FolderPlus,
  Hash,
  Loader2,
  Plus,
  Search,
  Trash2,
  Upload,
  X,
} from "lucide-react";
import GroupTree from "../components/knowledge/GroupTree";
import NoteList from "../components/knowledge/NoteList";
import MarkdownEditor from "../components/knowledge/MarkdownEditor";
import { useFileDrop } from "../hooks/useFileDrop";
import { useNoteDraft } from "../hooks/useNoteDraft";
import { pickDirectory, pickJsonFile, pickMarkdownFiles, pickSavePath } from "../lib/dialog";
import { countNotesByGroup, filterNotesByGroup } from "../lib/groupTree";
import { useKnowledgeStore } from "../stores/knowledgeStore";
import { cn } from "../lib/utils";
import type { Note, ViewMode } from "../types";

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
  const hits = useKnowledgeStore((s) => s.hits);
  const search = useKnowledgeStore((s) => s.search);
  const searchMode = useKnowledgeStore((s) => s.searchMode);
  const setSearchMode = useKnowledgeStore((s) => s.setSearchMode);
  const tags = useKnowledgeStore((s) => s.tags);
  const selectedTag = useKnowledgeStore((s) => s.selectedTag);
  const setSelectedTag = useKnowledgeStore((s) => s.setSelectedTag);
  const setNoteTags = useKnowledgeStore((s) => s.setNoteTags);
  const notice = useKnowledgeStore((s) => s.notice);
  const clearNotice = useKnowledgeStore((s) => s.clearNotice);
  const exportMarkdown = useKnowledgeStore((s) => s.exportMarkdown);
  const exportJson = useKnowledgeStore((s) => s.exportJson);
  const backupDatabase = useKnowledgeStore((s) => s.backupDatabase);
  const restoreBackup = useKnowledgeStore((s) => s.restoreBackup);

  const [viewMode, setViewMode] = useState<ViewMode>("split");

  const { isDragging } = useFileDrop((paths) => {
    importPaths(paths);
  });

  useEffect(() => {
    load();
  }, [load]);

  // 关键词变化后防抖触发全文检索
  useEffect(() => {
    const query = keyword.trim();
    if (!query) {
      search("");
      return;
    }
    const timer = window.setTimeout(() => {
      search(query);
    }, 250);
    return () => window.clearTimeout(timer);
  }, [keyword, search]);

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

  // 按标签筛选（在群组筛选结果之上叠加）
  const tagScopedNotes = useMemo(
    () =>
      selectedTag
        ? groupScopedNotes.filter((n) => n.tags.includes(selectedTag))
        : groupScopedNotes,
    [groupScopedNotes, selectedTag]
  );

  const searchActive = keyword.trim().length > 0;
  const searchScopedNotes = useMemo(() => {
    if (!searchActive) return tagScopedNotes;
    const byId = new Map(tagScopedNotes.map((n) => [n.id, n]));
    return hits
      .map((hit) => byId.get(hit.noteId))
      .filter((n): n is Note => Boolean(n));
  }, [searchActive, tagScopedNotes, hits]);

  const visibleNotes = isTrash ? trashNotes : searchScopedNotes;

  const handleImportFiles = () => {
    pickMarkdownFiles().then((paths) => importPaths(paths));
  };

  const handleImportFolder = () => {
    pickDirectory().then((dir) => (dir ? importPaths([dir]) : 0));
  };

  const stamp = new Date().toISOString().slice(0, 10);
  const handleExportMarkdown = () => {
    pickDirectory().then((dir) => {
      if (dir) exportMarkdown(dir);
    });
  };
  const handleExportJson = () => {
    pickSavePath(`ai-interview-backup-${stamp}.json`, "json").then((path) => {
      if (path) exportJson(path);
    });
  };
  const handleBackupDatabase = () => {
    pickSavePath(`ai-interview-${stamp}.sqlite`, "sqlite").then((path) => {
      if (path) backupDatabase(path);
    });
  };
  const handleRestore = () => {
    pickJsonFile().then((path) => {
      if (path) restoreBackup(path);
    });
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
              placeholder={
                searchMode === "semantic"
                  ? "语义检索相似笔记…"
                  : searchMode === "hybrid"
                    ? "混合检索（关键词 + 语义）…"
                    : "按标题 / 内容 / 标签检索…"
              }
              className="w-full border-0 bg-transparent p-0 text-[13px] text-text outline-none placeholder:text-text-subtle"
            />
          </div>

          {/* 检索模式切换：关键词（FTS5）/ 语义（向量） */}
          <div className="flex items-center gap-0.5 rounded-lg border border-white/10 bg-background-3/60 p-0.5">
            <button
              onClick={() => setSearchMode("keyword")}
              title="关键词检索（FTS5 + 中文分词）"
              className={cn(
                "cursor-pointer rounded-md px-2 py-1 text-[11px] transition-colors",
                searchMode === "keyword"
                  ? "bg-primary/20 text-primary-3"
                  : "text-text-subtle hover:text-text-muted"
              )}
            >
              关键词
            </button>
            <button
              onClick={() => setSearchMode("semantic")}
              title="语义检索（向量相似度）"
              className={cn(
                "cursor-pointer rounded-md px-2 py-1 text-[11px] transition-colors",
                searchMode === "semantic"
                  ? "bg-primary/20 text-primary-3"
                  : "text-text-subtle hover:text-text-muted"
              )}
            >
              语义
            </button>
            <button
              onClick={() => setSearchMode("hybrid")}
              title="混合检索（BM25 + 向量 RRF 融合）"
              className={cn(
                "cursor-pointer rounded-md px-2 py-1 text-[11px] transition-colors",
                searchMode === "hybrid"
                  ? "bg-primary/20 text-primary-3"
                  : "text-text-subtle hover:text-text-muted"
              )}
            >
              混合
            </button>
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

          {/* 导出 / 备份 / 恢复 */}
          <div className="flex items-center gap-0.5 rounded-lg border border-white/10 bg-background-3/60 p-0.5">
            <button
              title="导出全部笔记为 Markdown"
              onClick={handleExportMarkdown}
              className="cursor-pointer rounded-md px-1.5 py-1 text-text-subtle transition-colors hover:bg-white/10 hover:text-text"
            >
              <FileDown className="h-3.5 w-3.5" />
            </button>
            <button
              title="导出 JSON 备份"
              onClick={handleExportJson}
              className="cursor-pointer rounded-md px-1.5 py-1 text-text-subtle transition-colors hover:bg-white/10 hover:text-text"
            >
              <FileJson className="h-3.5 w-3.5" />
            </button>
            <button
              title="备份数据库文件"
              onClick={handleBackupDatabase}
              className="cursor-pointer rounded-md px-1.5 py-1 text-text-subtle transition-colors hover:bg-white/10 hover:text-text"
            >
              <Database className="h-3.5 w-3.5" />
            </button>
            <button
              title="从 JSON 备份恢复"
              onClick={handleRestore}
              className="cursor-pointer rounded-md px-1.5 py-1 text-text-subtle transition-colors hover:bg-white/10 hover:text-text"
            >
              <Upload className="h-3.5 w-3.5" />
            </button>
          </div>
        </div>
      </div>

      {error && (
        <div className="border-b border-danger/20 bg-danger/10 px-4 py-1.5 text-[11px] text-danger">
          {error}
        </div>
      )}

      {notice && (
        <div className="flex items-center gap-2 border-b border-success/20 bg-success/10 px-4 py-1.5 text-[11px] text-success">
          <CircleCheck className="h-3 w-3 shrink-0" />
          <span className="min-w-0 flex-1 truncate">{notice}</span>
          <button
            title="关闭提示"
            onClick={clearNotice}
            className="cursor-pointer text-success/70 hover:text-success"
          >
            <X className="h-3 w-3" />
          </button>
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

            {!isTrash && tags.length > 0 && (
              <div className="mt-3">
                <div className="flex items-center gap-1 px-1 pb-1 text-[10px] uppercase tracking-wide text-text-subtle">
                  <Hash className="h-3 w-3" />
                  标签
                  {selectedTag && (
                    <button
                      title="清除标签筛选"
                      onClick={() => setSelectedTag(null)}
                      className="ml-auto cursor-pointer text-[10px] text-primary-3 hover:underline"
                    >
                      清除
                    </button>
                  )}
                </div>
                <div className="flex flex-wrap gap-1 px-1">
                  {tags.map((tag) => {
                    const active = selectedTag === tag.name;
                    return (
                      <button
                        key={tag.id}
                        onClick={() => setSelectedTag(active ? null : tag.name)}
                        className={cn(
                          "cursor-pointer rounded-full border px-2 py-0.5 text-[11px] transition-colors",
                          active
                            ? "border-primary/40 bg-primary/20 text-primary-3"
                            : "border-white/10 bg-white/[0.04] text-text-subtle hover:text-text-muted"
                        )}
                      >
                        {tag.name}
                      </button>
                    );
                  })}
                </div>
              </div>
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
                hits={!isTrash && searchActive ? hits : undefined}
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
              tags={selected.tags}
              tagSuggestions={tags.map((t) => t.name)}
              onTitleChange={draft.setTitle}
              onContentChange={draft.setContent}
              onViewModeChange={setViewMode}
              onAddTag={(tag) => setNoteTags(selected.id, [...selected.tags, tag])}
              onRemoveTag={(tag) =>
                setNoteTags(
                  selected.id,
                  selected.tags.filter((t) => t !== tag)
                )
              }
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
