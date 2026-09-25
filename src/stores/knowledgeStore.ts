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
  error: string | null;
  selectedId: string | null;
  keyword: string;
  /** 加载笔记与群组 */
  load: () => Promise<void>;
  /** 导入 Markdown 路径，返回成功导入数量 */
  importPaths: (paths: string[]) => Promise<number>;
  select: (id: string | null) => void;
  setKeyword: (keyword: string) => void;
}

export const useKnowledgeStore = create<KnowledgeState>((set, get) => ({
  notes: [],
  groups: [],
  loading: false,
  importing: false,
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

  select: (id) => set({ selectedId: id }),
  setKeyword: (keyword) => set({ keyword }),
}));
