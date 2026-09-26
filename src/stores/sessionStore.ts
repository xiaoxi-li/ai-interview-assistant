import { create } from "zustand";
import { ipc } from "../lib/ipc";
import { pickSavePath } from "../lib/dialog";
import type { SessionDetail, SessionExportSummary, SessionSummary } from "../types";

export interface SessionNotice {
  kind: "success" | "error";
  text: string;
}

interface SessionState {
  /** 会话列表 */
  sessions: SessionSummary[];
  /** 当前选中的会话 ID */
  selectedId: string | null;
  /** 当前会话详情（转写 + 问答） */
  detail: SessionDetail | null;
  loading: boolean;
  detailLoading: boolean;
  exporting: boolean;
  notice: SessionNotice | null;

  loadSessions: () => Promise<void>;
  select: (id: string) => Promise<void>;
  loadDetail: (id: string) => Promise<void>;
  deleteSession: (id: string) => Promise<void>;
  exportSession: (id: string, format: "markdown" | "json") => Promise<void>;
  clearNotice: () => void;
}

export const useSessionStore = create<SessionState>((set, get) => {
  const fail = (action: string, err: unknown) => {
    console.error(`${action}失败:`, err);
    set({
      loading: false,
      detailLoading: false,
      exporting: false,
      notice: { kind: "error", text: String(err) },
    });
  };

  return {
    sessions: [],
    selectedId: null,
    detail: null,
    loading: false,
    detailLoading: false,
    exporting: false,
    notice: null,

    loadSessions: () => {
      set({ loading: true });
      return ipc<SessionSummary[]>("list_sessions", { limit: 100 })
        .then((sessions) => {
          set({ sessions, loading: false });
          // 首次进入或选中项已失效时，自动选中最近一场
          const current = get().selectedId;
          const stillExists = current && sessions.some((session) => session.id === current);
          if (stillExists) return;
          const first = sessions[0];
          if (first) {
            return get().select(first.id);
          }
          set({ selectedId: null, detail: null });
        })
        .catch((err) => fail("读取会话列表", err));
    },

    select: (id) => {
      set({ selectedId: id });
      return get().loadDetail(id);
    },

    loadDetail: (id) => {
      set({ detailLoading: true });
      return ipc<SessionDetail>("get_session_detail", { sessionId: id })
        .then((detail) => set({ detail, detailLoading: false }))
        .catch((err) => {
          console.error("读取会话详情失败:", err);
          set({ detailLoading: false, detail: null });
          fail("读取会话详情", err);
        });
    },

    deleteSession: (id) => {
      return ipc<boolean>("delete_session", { sessionId: id })
        .then(() => {
          set((state) => ({
            sessions: state.sessions.filter((session) => session.id !== id),
            selectedId: state.selectedId === id ? null : state.selectedId,
            detail: state.selectedId === id ? null : state.detail,
            notice: { kind: "success", text: "会话已删除" },
          }));
          return get().loadSessions();
        })
        .catch((err) => fail("删除会话", err));
    },

    exportSession: (id, format) => {
      const detail = get().detail;
      const title = detail?.session.id === id ? detail.session.title : "面试会话";
      const ext = format === "json" ? "json" : "md";

      return pickSavePath(`${title}.${ext}`, ext)
        .then((dest) => {
          if (!dest) return undefined;
          set({ exporting: true });
          return ipc<SessionExportSummary>("export_session", {
            sessionId: id,
            dest,
            format,
          })
            .then((summary) =>
              set({
                exporting: false,
                notice: {
                  kind: "success",
                  text: `已导出 ${summary.format}（转写 ${summary.segments} 条 / 问答 ${summary.questions} 组）→ ${summary.path}`,
                },
              })
            )
            .catch((err) => fail("导出会话", err));
        })
        .catch((err) => fail("选择导出路径", err));
    },

    clearNotice: () => set({ notice: null }),
  };
});
