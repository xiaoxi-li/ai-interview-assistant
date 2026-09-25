import { create } from "zustand";
import { ipc } from "../lib/ipc";
import type { Group, Note } from "../types";

interface ImportOutcome {
  imported: number;
  skipped: string[];
  notes: Note[];
}

interface KnowledgeState {
  notes: Note[];
  groups: Group[];
  loading: boolean;
  importing: boolean;
  saving: boolean;
  savedAt: number | null;
  error: string | null;
  selectedId: string | null;
  keyword: string;
  /** 加载笔记与群组 */
  load: () => Promise<void>;
  /** 导入 Markdown 路径，返回成功导入数量 */
  importPaths: (paths: string[]) => Promise<number>;
  /** 保存笔记标题与正文 */
  updateNote: (id: string, title: string, content: string) => Promise<void>;
  select: (id: string | null) => void;
  setKeyword: (keyword: string) => void;
}

export const useKnowledgeStore = create<KnowledgeState>((set, get) => ({
  notes: [],
  groups: [],
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
      ipc<Group[]>("list_groups"),
    ])
      .then(([notes, groups]) => {
        const current = get().selectedId;
        const stillExists = current && notes.some((n) => n.id === current);
        set({
          notes,
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
            set({ importing: false });
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

  select: (id) => set({ selectedId: id }),
  setKeyword: (keyword) => set({ keyword }),
}));
