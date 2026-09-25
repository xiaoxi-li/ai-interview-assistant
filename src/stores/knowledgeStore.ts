import { create } from "zustand";
import { ipc } from "../lib/ipc";
import type { Group, Note, SearchHit, Tag } from "../types";

interface ImportOutcome {
  imported: number;
  skipped: string[];
  notes: Note[];
}

export type KnowledgeView = "all" | "trash";
/** keyword = FTS5 全文检索；semantic = 向量语义检索；hybrid = 双路 RRF 融合 */
export type SearchMode = "keyword" | "semantic" | "hybrid";

const SEARCH_COMMANDS: Record<SearchMode, { command: string; key: string }> = {
  keyword: { command: "search_notes", key: "limit" },
  semantic: { command: "search_similar_notes", key: "topK" },
  hybrid: { command: "search_hybrid_notes", key: "topK" },
};
/** undefined = 全部笔记；null = 未分组 */
export type SelectedGroup = string | null | undefined;

interface KnowledgeState {
  notes: Note[];
  trashNotes: Note[];
  groups: Group[];
  tags: Tag[];
  view: KnowledgeView;
  selectedGroupId: SelectedGroup;
  /** 标签筛选（null 表示不筛选） */
  selectedTag: string | null;
  loading: boolean;
  importing: boolean;
  saving: boolean;
  savedAt: number | null;
  error: string | null;
  selectedId: string | null;
  keyword: string;
  /** 检索命中结果（关键词为空时清空） */
  hits: SearchHit[];
  searchMode: SearchMode;
  /** 加载笔记、回收站与群组 */
  load: () => Promise<void>;
  /** 按当前检索模式执行检索 */
  search: (keyword: string) => Promise<void>;
  setSearchMode: (mode: SearchMode) => void;
  /** 导入 Markdown 路径，返回成功导入数量 */
  importPaths: (paths: string[]) => Promise<number>;
  /** 保存笔记标题与正文 */
  updateNote: (id: string, title: string, content: string) => Promise<void>;
  /** 新建空白笔记并选中；可指定归属群组 */
  createNote: (groupId?: string | null) => Promise<void>;
  trashNote: (id: string) => Promise<void>;
  restoreNote: (id: string) => Promise<void>;
  purgeNote: (id: string) => Promise<void>;
  emptyTrash: () => Promise<void>;
  moveNote: (id: string, groupId: string | null) => Promise<void>;
  /** 覆盖设置笔记标签 */
  setNoteTags: (noteId: string, tags: string[]) => Promise<void>;
  /** 导出全部笔记为 Markdown 到目录 */
  exportMarkdown: (dest: string) => Promise<void>;
  /** 导出 JSON 逻辑备份 */
  exportJson: (dest: string) => Promise<void>;
  /** 数据库文件级备份 */
  backupDatabase: (dest: string) => Promise<void>;
  /** 从 JSON 备份恢复 */
  restoreBackup: (source: string) => Promise<void>;
  /** 操作提示（成功/失败信息） */
  notice: string | null;
  clearNotice: () => void;
  createGroup: (name: string, parentId: string | null) => Promise<void>;
  renameGroup: (id: string, name: string) => Promise<void>;
  deleteGroup: (id: string) => Promise<void>;
  moveGroup: (id: string, parentId: string | null) => Promise<void>;
  reorderGroups: (orderedIds: string[]) => Promise<void>;
  setView: (view: KnowledgeView) => void;
  setSelectedGroup: (groupId: SelectedGroup) => void;
  setSelectedTag: (tag: string | null) => void;
  select: (id: string | null) => void;
  setKeyword: (keyword: string) => void;
}

export const useKnowledgeStore = create<KnowledgeState>((set, get) => {
  /** 执行无需额外前后处理的命令，完成后刷新数据 */
  const runAction = (command: string, args?: Record<string, unknown>) =>
    ipc(command, args)
      .then(() => get().load())
      .catch((err) => {
        console.error(`${command} 调用失败:`, err);
        set({ error: String(err) });
      });

  return {
    notes: [],
    trashNotes: [],
    groups: [],
    tags: [],
    view: "all",
    selectedGroupId: undefined,
    selectedTag: null,
    loading: false,
    importing: false,
    saving: false,
    savedAt: null,
    error: null,
    selectedId: null,
    keyword: "",
    hits: [],
    searchMode: "keyword",
    notice: null,

    search: (keyword) => {
      const query = keyword.trim();
      if (!query) {
        set({ hits: [] });
        return Promise.resolve();
      }
      const { command, key } = SEARCH_COMMANDS[get().searchMode];
      const args = { query, [key]: key === "limit" ? 50 : 20 };
      return ipc<SearchHit[]>(command, args)
        .then((hits) => set({ hits }))
        .catch((err) => {
          console.error("检索失败:", err);
          set({ hits: [], error: String(err) });
        });
    },

    setSearchMode: (searchMode) => {
      set({ searchMode });
      // 切换模式后立即用当前关键词重查
      const keyword = get().keyword;
      if (keyword.trim()) {
        get().search(keyword);
      }
    },

    load: () => {
      set({ loading: true, error: null });
      return Promise.all([
        ipc<Note[]>("list_notes"),
        ipc<Note[]>("list_trash_notes"),
        ipc<Group[]>("list_groups"),
        ipc<Tag[]>("list_tags"),
      ])
        .then(([notes, trashNotes, groups, tags]) => {
          const current = get().selectedId;
          const stillExists = current && notes.some((n) => n.id === current);
          set({
            notes,
            trashNotes,
            groups,
            tags,
            loading: false,
            selectedId: stillExists ? current : (notes[0]?.id ?? null),
          });
        })
        .catch((err) => {
          console.error("加载知识库失败:", err);
          set({ loading: false, error: String(err) });
        });
    },

    importPaths: (paths) => {
      if (paths.length === 0) return Promise.resolve(0);
      const groupId = get().selectedGroupId;
      set({ importing: true, error: null });
      return ipc<ImportOutcome>("import_markdown", {
        paths,
        groupId: typeof groupId === "string" ? groupId : null,
      })
        .then((outcome) => {
          if (outcome.skipped.length > 0) {
            console.warn("部分文件未导入:", outcome.skipped);
          }
          return get()
            .load()
            .then(() => {
              set({ importing: false, view: "all" });
              return outcome.imported;
            });
        })
        .catch((err) => {
          console.error("导入失败:", err);
          set({ importing: false, error: String(err) });
          return 0;
        });
    },

    updateNote: (id, title, content) => {
      set({ saving: true, error: null });
      return ipc<Note>("update_note", { id, title, content })
        .then((updated) => {
          set((state) => ({
            saving: false,
            savedAt: Date.now(),
            // 局部替换，避免整表刷新导致编辑器抖动
            notes: state.notes.map((n) => (n.id === updated.id ? updated : n)),
          }));
        })
        .catch((err) => {
          console.error("保存笔记失败:", err);
          set({ saving: false, error: String(err) });
        });
    },

    createNote: (groupId) => {
      const target =
        groupId !== undefined
          ? groupId
          : typeof get().selectedGroupId === "string"
            ? (get().selectedGroupId as string)
            : null;
      return ipc<Note>("create_note", { groupId: target })
        .then((note) =>
          get()
            .load()
            .then(() => set({ selectedId: note.id, view: "all" }))
        )
        .catch((err) => {
          console.error("新建笔记失败:", err);
          set({ error: String(err) });
        });
    },

    trashNote: (id) => {
      return ipc<boolean>("delete_note", { id })
        .then(() => {
          set((state) => ({
            selectedId: state.selectedId === id ? null : state.selectedId,
          }));
          return get().load();
        })
        .catch((err) => {
          console.error("删除笔记失败:", err);
          set({ error: String(err) });
        });
    },

    restoreNote: (id) => runAction("restore_note", { id }),
    purgeNote: (id) => runAction("purge_note", { id }),
    emptyTrash: () => runAction("empty_trash"),
    moveNote: (id, groupId) => runAction("move_note", { id, groupId }),

    setNoteTags: (noteId, tags) => {
      set({ saving: true, error: null });
      return ipc<Note>("set_note_tags", { id: noteId, tags })
        .then((updated) => {
          set((state) => ({
            saving: false,
            notes: state.notes.map((n) => (n.id === updated.id ? updated : n)),
          }));
          // 标签集合可能新增，刷新标签列表
          return ipc<Tag[]>("list_tags")
            .then((tags) => set({ tags }))
            .catch((err) => console.error("刷新标签失败:", err));
        })
        .catch((err) => {
          console.error("设置标签失败:", err);
          set({ saving: false, error: String(err) });
        });
    },
    createGroup: (name, parentId) => runAction("create_group", { name, parentId }),
    renameGroup: (id, name) => runAction("rename_group", { id, name }),
    deleteGroup: (id) => {
      const current = get().selectedGroupId;
      return runAction("delete_group", { id }).then(() => {
        if (current === id) set({ selectedGroupId: undefined });
      });
    },
    moveGroup: (id, parentId) => runAction("move_group", { id, parentId }),
    reorderGroups: (orderedIds) => runAction("reorder_groups", { orderedIds }),

    exportMarkdown: (dest) => {
      set({ error: null });
      return ipc<{ files: number; dest: string }>("export_notes_markdown", { dest })
        .then((summary) =>
          set({ notice: `已导出 ${summary.files} 篇笔记到 ${summary.dest}` })
        )
        .catch((err) => {
          console.error("导出 Markdown 失败:", err);
          set({ error: String(err) });
        });
    },

    exportJson: (dest) => {
      set({ error: null });
      return ipc<{ files: number; dest: string }>("export_backup_json", { dest })
        .then((summary) =>
          set({ notice: `已导出 JSON 备份（${summary.files} 篇）到 ${summary.dest}` })
        )
        .catch((err) => {
          console.error("导出 JSON 失败:", err);
          set({ error: String(err) });
        });
    },

    backupDatabase: (dest) => {
      set({ error: null });
      return ipc<{ files: number; dest: string }>("backup_database", { dest })
        .then((summary) => set({ notice: `数据库备份完成：${summary.dest}` }))
        .catch((err) => {
          console.error("数据库备份失败:", err);
          set({ error: String(err) });
        });
    },

    restoreBackup: (source) => {
      set({ error: null });
      return ipc<{ groups: number; tags: number; notes: number }>("restore_backup", {
        source,
      })
        .then((summary) => {
          set({
            notice: `恢复完成：新增 ${summary.notes} 篇笔记、${summary.groups} 个群组`,
          });
          return get().load();
        })
        .catch((err) => {
          console.error("恢复备份失败:", err);
          set({ error: String(err) });
        });
    },

    clearNotice: () => set({ notice: null }),

    setView: (view) => set({ view }),
    setSelectedGroup: (selectedGroupId) => set({ selectedGroupId }),
    setSelectedTag: (selectedTag) => set({ selectedTag }),
    select: (id) => set({ selectedId: id }),
    setKeyword: (keyword) => set({ keyword }),
  };
});
