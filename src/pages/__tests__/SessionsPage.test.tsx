import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import SessionsPage from "../SessionsPage";
import { useSessionStore } from "../../stores/sessionStore";
import type { SessionDetail, SessionSummary } from "../../types";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

const startedAt = 1_700_000_000_000;

const session: SessionSummary = {
  id: "s-1",
  title: "面试会话 · 双通道",
  startedAt,
  endedAt: startedAt + 1_200_000,
  segmentCount: 2,
  questionCount: 1,
};

const detail: SessionDetail = {
  session: { id: "s-1", title: "面试会话 · 双通道", startedAt, endedAt: startedAt + 1_200_000 },
  segments: [
    {
      id: "t1",
      sessionId: "s-1",
      speaker: "interviewer",
      text: "介绍一下你最近负责的项目",
      isFinal: true,
      ts: startedAt + 12_000,
    },
    {
      id: "t2",
      sessionId: "s-1",
      speaker: "me",
      text: "我负责桌面端应用的架构设计",
      isFinal: true,
      ts: startedAt + 20_000,
    },
  ],
  qa: [
    {
      id: "q1",
      sessionId: "s-1",
      question: "介绍一下你最近负责的项目",
      answer: "我负责桌面端应用架构设计。",
      sources: [{ noteId: "n-1", title: "项目架构设计" }],
      ts: startedAt + 13_000,
    },
  ],
};

function resetStore(overrides: Partial<ReturnType<typeof useSessionStore.getState>> = {}) {
  useSessionStore.setState({
    sessions: [session],
    selectedId: "s-1",
    detail,
    loading: false,
    detailLoading: false,
    exporting: false,
    notice: null,
    loadSessions: vi.fn().mockResolvedValue(undefined),
    select: vi.fn().mockResolvedValue(undefined),
    loadDetail: vi.fn().mockResolvedValue(undefined),
    deleteSession: vi.fn().mockResolvedValue(undefined),
    exportSession: vi.fn().mockResolvedValue(undefined),
    clearNotice: vi.fn(),
    ...overrides,
  });
}

describe("SessionsPage", () => {
  beforeEach(() => {
    resetStore();
  });

  it("进入页面加载会话列表", () => {
    render(<SessionsPage />);
    expect(useSessionStore.getState().loadSessions).toHaveBeenCalled();
  });

  it("无会话时给出引导", () => {
    resetStore({ sessions: [], selectedId: null, detail: null });
    render(<SessionsPage />);

    expect(screen.getByText(/暂无面试会话/)).toBeTruthy();
    expect(screen.getByText(/选择左侧会话查看完整回放/)).toBeTruthy();
  });

  it("展示会话列表与计数", () => {
    render(<SessionsPage />);
    expect(screen.getByText("共 1 场")).toBeTruthy();
    expect(screen.getByText("转写 2 条 · 问答 1 组")).toBeTruthy();
  });

  it("点击会话切换详情", () => {
    const select = vi.fn().mockResolvedValue(undefined);
    resetStore({ select });
    render(<SessionsPage />);

    // 列表项与详情标题同名，取列表中的第一个
    fireEvent.click(screen.getAllByText("面试会话 · 双通道")[0]);
    expect(select).toHaveBeenCalledWith("s-1");
  });

  it("回放展示转写时间线与说话人", () => {
    render(<SessionsPage />);

    expect(screen.getByText("转写时间线（2 条）")).toBeTruthy();
    expect(screen.getByText("介绍一下你最近负责的项目")).toBeTruthy();
    expect(screen.getByText("我负责桌面端应用的架构设计")).toBeTruthy();
    expect(screen.getByText("面试官")).toBeTruthy();
    expect(screen.getByText("我")).toBeTruthy();
    // 相对会话开始的偏移
    expect(screen.getByText("00:12")).toBeTruthy();
    expect(screen.getByText("00:20")).toBeTruthy();
  });

  it("回放展示问答与引用来源", () => {
    render(<SessionsPage />);

    expect(screen.getByText("问答记录（1 组）")).toBeTruthy();
    expect(screen.getByText(/Q1\. 介绍一下你最近负责的项目/)).toBeTruthy();
    expect(screen.getByText("我负责桌面端应用架构设计。")).toBeTruthy();
    expect(screen.getByText("项目架构设计")).toBeTruthy();
  });

  it("未生成回答时给出占位", () => {
    resetStore({
      detail: { ...detail, qa: [{ ...detail.qa[0], answer: "" }] },
    });
    render(<SessionsPage />);
    expect(screen.getByText("（未生成回答）")).toBeTruthy();
  });

  it("导出 Markdown / JSON 触发对应动作", () => {
    const exportSession = vi.fn().mockResolvedValue(undefined);
    resetStore({ exportSession });
    render(<SessionsPage />);

    fireEvent.click(screen.getByText("导出 Markdown"));
    expect(exportSession).toHaveBeenCalledWith("s-1", "markdown");

    fireEvent.click(screen.getByText("导出 JSON"));
    expect(exportSession).toHaveBeenLastCalledWith("s-1", "json");
  });

  it("删除需二次确认后才执行", () => {
    const deleteSession = vi.fn().mockResolvedValue(undefined);
    resetStore({ deleteSession });
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(false);

    render(<SessionsPage />);
    fireEvent.click(screen.getByText("删除"));
    expect(deleteSession).not.toHaveBeenCalled();

    confirmSpy.mockReturnValue(true);
    fireEvent.click(screen.getByText("删除"));
    expect(deleteSession).toHaveBeenCalledWith("s-1");

    confirmSpy.mockRestore();
  });

  it("展示提示条并可关闭", () => {
    const clearNotice = vi.fn();
    resetStore({
      clearNotice,
      notice: { kind: "success", text: "已导出 markdown（转写 2 条 / 问答 1 组）" },
    });
    render(<SessionsPage />);

    expect(screen.getByText(/已导出 markdown/)).toBeTruthy();
    fireEvent.click(screen.getByTitle("关闭提示"));
    expect(clearNotice).toHaveBeenCalled();
  });

  it("未结束的会话标注状态", () => {
    resetStore({
      sessions: [{ ...session, id: "s-2", title: "进行中的会话", endedAt: null }],
      selectedId: "s-2",
    });
    render(<SessionsPage />);

    expect(screen.getByText("未结束")).toBeTruthy();
  });
});
