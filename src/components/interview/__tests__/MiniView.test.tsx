import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import MiniView from "../MiniView";
import { useInterviewStore } from "../../../stores/interviewStore";
import type { AsrSessionStatus, DetectedQuestion, QaPair, RecallHit } from "../../../types";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

const ipcMock = vi.fn();
vi.mock("../../../lib/ipc", () => ({
  ipc: (...args: unknown[]) => ipcMock(...args),
}));

const closeMiniWindow = vi.fn().mockResolvedValue(undefined);
vi.mock("../../../lib/window", () => ({
  closeMiniWindow: () => closeMiniWindow(),
  openMiniWindow: vi.fn(),
  isMiniWindow: () => true,
  MINI_WINDOW_LABEL: "mini",
}));

const runningStatus: AsrSessionStatus = {
  running: true,
  sessionId: "s-1",
  source: "system",
  speaker: "interviewer",
  engine: "sherpa-onnx",
  startedAt: 1000,
  paused: false,
};

const question: DetectedQuestion = {
  text: "介绍一下你最近负责的项目",
  score: 1.05,
  signals: [],
  ts: Date.now(),
};

const hit: RecallHit = {
  noteId: "note-1",
  title: "项目架构设计",
  snippet: "负责桌面应用的架构设计",
  score: 0.03,
};

function resetStore(overrides: Partial<ReturnType<typeof useInterviewStore.getState>> = {}) {
  useInterviewStore.setState({
    currentQuestion: null,
    currentQaId: null,
    answerText: "",
    answerStatus: "idle",
    answerError: null,
    sources: [],
    applyQuestionPush: vi.fn(),
    applyAnswerPush: vi.fn(),
    loadAsrStatus: vi.fn().mockResolvedValue(undefined),
    hydrateFromPair: vi.fn(),
    generateAnswer: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  });
}

describe("MiniView", () => {
  beforeEach(() => {
    resetStore();
    ipcMock.mockReset();
    ipcMock.mockResolvedValue(null);
    closeMiniWindow.mockClear();
  });

  it("无内容时展示等待提示", () => {
    render(<MiniView />);
    expect(screen.getByText(/等待识别面试官问题/)).toBeTruthy();
    expect(screen.getByText(/识别到问题后这里会显示 AI 回答/)).toBeTruthy();
  });

  it("展示当前问题与流式回答", () => {
    resetStore({
      currentQuestion: question,
      currentQaId: "q1",
      answerText: "我最近负责桌面端应用的架构设计",
      answerStatus: "streaming",
      sources: [hit],
    });
    render(<MiniView />);

    expect(screen.getByText("介绍一下你最近负责的项目")).toBeTruthy();
    expect(screen.getByText(/我最近负责桌面端应用的架构设计/)).toBeTruthy();
    expect(screen.getByText("项目架构设计")).toBeTruthy();
  });

  it("展示生成失败信息", () => {
    resetStore({
      currentQuestion: question,
      answerError: "尚未配置 API Key",
    });
    render(<MiniView />);

    expect(screen.getByText("尚未配置 API Key")).toBeTruthy();
  });

  it("打开时拉取会话状态与最近问答以恢复显示", async () => {
    const hydrateFromPair = vi.fn();
    resetStore({
      hydrateFromPair,
      loadAsrStatus: vi.fn().mockImplementation(() => {
        useInterviewStore.setState({ asrStatus: runningStatus });
        return Promise.resolve();
      }),
    });
    ipcMock.mockResolvedValue({
      id: "q1",
      sessionId: "s-1",
      question: "上一个问题",
      answer: "上一段回答",
      sources: [],
      ts: 1,
    } satisfies QaPair);

    render(<MiniView />);

    await waitFor(() => expect(hydrateFromPair).toHaveBeenCalled());
    expect(ipcMock).toHaveBeenCalledWith("latest_question", { sessionId: "s-1" });
  });

  it("返回主窗口按钮关闭浮窗", () => {
    render(<MiniView />);
    fireEvent.click(screen.getByTitle("返回主窗口"));
    expect(closeMiniWindow).toHaveBeenCalledTimes(1);
  });

  it("无当前问题时禁用重新生成", () => {
    resetStore({ asrStatus: runningStatus });
    render(<MiniView />);

    const button = screen.getByTitle("重新生成回答") as HTMLButtonElement;
    expect(button.disabled).toBe(true);
  });

  it("有会话与问题时可用重新生成", () => {
    const generateAnswer = vi.fn().mockResolvedValue(undefined);
    resetStore({
      asrStatus: runningStatus,
      currentQuestion: question,
      currentQaId: "q1",
      generateAnswer,
    });
    render(<MiniView />);

    fireEvent.click(screen.getByTitle("重新生成回答"));
    expect(generateAnswer).toHaveBeenCalledWith("s-1", "q1", true);
  });
});
