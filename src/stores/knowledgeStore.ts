import { create } from "zustand";
import { ipc } from "../lib/ipc";
import type { Group, Note } from "../types";

interface ImportOutcome {
  imported: number;
  skipped: string[];
  notes: Note[];
}

export type KnowledgeView = "all" | "trash";

interface KnowledgeState {
  notes: Note[];
  trashNotes: Note[];
  groups: Group[];
  view: KnowledgeView;
  loading: boolean;
  importing: boolean;
  saving: boolean;
  savedAt: number | null;
  error: string | null;
  selectedId: string | null;
  keyword: string;
  /** 加载笔记、回收站与群组 */
  load: () => Promise<void>;
  /** 导入 Markdown 路径，返回成功导入数量 */
  importPaths: (paths: string[]) => Promise<number>;
  /** 保存笔记标题与正文 */
  updateNote: (id: string, title: string, content: string) => Promise<void>;
  /** 新建空白笔记并选中 */
  createNote: () => Promise<void>;
  trashNote: (id: string) => Promise<void>;
  restoreNote: (id: string) => Promise<void>;
  purgeNote: (id: string) => Promise<void>;
  emptyTrash: () => Promise<void>;
  moveNote: (id: string, groupId: string | null) => Promise<void>;
  setView: (view: KnowledgeView) => void;
  select: (id: string | null) => void;
  setKeyword: (keyword: string) => void;
}

export const useKnowledgeStore = create<KnowledgeState>((set, get) => ({
  notes: [],
  trashNotes: [],
  groups: [],
  view: "all",
  loading: false,
  importing: false,
  saving: false,
  savedAt: null,
  error: null,
  selectedId: null,
  keyword: "",

  load: () => {
    set({ loading: true, error: null });
    return Promise.all([
      ipc<Note[]>("list_notes"),
      ipc<Note[]>("list_trash_notes"),
      ipc<Group[]>("list_groups"),
    ])
      .then(([notes, trashNotes, groups]) => {
        const current = get().selectedId;
        const stillExists = current && notes.some((n) => n.id === current);
        set({
          notes,
          trashNotes,
          groups,
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
    set({ importing: true, error: null });
    return ipc<ImportOutcome>("import_markdown", { paths })
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

  createNote: () => {
    return ipc<Note>("create_note", { groupId: null })
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

  restoreNote: (id) => {
    return ipc<boolean>("restore_note", { id })
      .then(() => get().load())
      .catch((err) => {
        console.error("恢复笔记失败:", err);
        set({ error: String(err) });
      });
  },

  purgeNote: (id) => {
    return ipc<boolean>("purge_note", { id })
      .then(() => get().load())
      .catch((err) => {
        console.error("彻底删除失败:", err);
        set({ error: String(err) });
      });
  },

  emptyTrash: () => {
    return ipc<number>("empty_trash")
      .then(() => get().load())
      .catch((err) => {
        console.error("清空回收站失败:", err);
        set({ error: String(err) });
      });
  },

  moveNote: (id, groupId) => {
    return ipc<boolean>("move_note", { id, groupId })
      .then(() => get().load())
      .catch((err) => {
        console.error("移动笔记失败:", err);
        set({ error: String(err) });
      });
  },

  setView: (view) => set({ view }),
  select: (id) => set({ selectedId: id }),
  setKeyword: (keyword) => set({ keyword }),
}));
