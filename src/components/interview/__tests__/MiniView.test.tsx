import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import MiniView from "../MiniView";
import { useAppStore } from "../../../stores/appStore";
import { useInterviewStore, type TranscriptLine } from "../../../stores/interviewStore";
import type {
  AsrSessionStatus,
  DetectedQuestion,
  QaPair,
  QuestionConfig,
  RecallHit,
} from "../../../types";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

const ipcMock = vi.fn();
vi.mock("../../../lib/ipc", () => ({
  ipc: (...args: unknown[]) => ipcMock(...args),
}));

const closeWindow = vi.fn().mockResolvedValue(undefined);
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ close: () => closeWindow() }),
}));

// 「退出应用」仅在 Tauri 环境下真正调用窗口 API，这里固定为 Tauri 环境以便断言
vi.mock("../../../lib/utils", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../../lib/utils")>();
  return { ...actual, isTauri: () => true };
});

const runningStatus: AsrSessionStatus = {
  running: true,
  sessionId: "s-1",
  source: "system",
  speaker: "interviewer",
  engine: "sherpa-onnx",
  startedAt: 1000,
  paused: false,
  dual: false,
  aecEnabled: false,
  aecErleDb: null,
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

const autoAnswerOff: QuestionConfig = {
  autoDetect: true,
  threshold: 0.5,
  minChars: 4,
  mergeWindowMs: 3000,
  autoAnswer: false,
};

/** 转写行夹具（mini 页只消费面试官一侧） */
function line(overrides: Partial<TranscriptLine> = {}): TranscriptLine {
  return {
    id: "t1",
    speaker: "interviewer",
    text: "请你介绍一下你最近负责的项目",
    isFinal: true,
    ts: 1000,
    ...overrides,
  };
}

const exitMiniMode = vi.fn();

function resetStore(overrides: Partial<ReturnType<typeof useInterviewStore.getState>> = {}) {
  useAppStore.setState({ miniMode: true, exitMiniMode });

  useInterviewStore.setState({
    asrStatus: null,
    currentQuestion: null,
    currentQaId: null,
    transcript: [],
    partial: null,
    answerText: "",
    answerStatus: "idle",
    answerError: null,
    sources: [],
    persistedSources: [],
    questionConfig: null,
    applyQuestionPush: vi.fn(),
    applyAnswerPush: vi.fn(),
    applyTranscriptPush: vi.fn(),
    loadAsrStatus: vi.fn().mockResolvedValue(undefined),
    hydrateFromPair: vi.fn(),
    generateAnswer: vi.fn().mockResolvedValue(undefined),
    loadQuestionConfig: vi.fn().mockResolvedValue(undefined),
    recallSources: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  });
}

describe("MiniView", () => {
  beforeEach(() => {
    resetStore();
    ipcMock.mockReset();
    ipcMock.mockResolvedValue(null);
    exitMiniMode.mockClear();
    closeWindow.mockClear();
  });

  it("无内容时展示等待提示", () => {
    render(<MiniView />);
    expect(screen.getByText(/等待识别面试官问题/)).toBeTruthy();
    expect(screen.getByText(/识别到问题后这里会显示 AI 回答/)).toBeTruthy();
  });

  it("顶栏不再出现「迷你」文字（品牌标识 + 恢复/退出两个图标）", () => {
    render(<MiniView />);
    expect(screen.queryByText("迷你")).toBeNull();
    expect(screen.queryByText("Mini")).toBeNull();
    expect(screen.getByTitle("恢复完整界面")).toBeTruthy();
    expect(screen.getByTitle("退出应用")).toBeTruthy();
  });

  it("未识别出问题时展示面试官 partial（逐字中间结果）", () => {
    resetStore({ partial: line({ id: "partial", text: "请你介绍一下", isFinal: false }) });
    render(<MiniView />);

    expect(screen.getByText("请你介绍一下…")).toBeTruthy();
    expect(screen.queryByText(/等待识别面试官问题/)).toBeNull();
  });

  it("无 partial 时展示最近一条面试官 final", () => {
    resetStore({
      transcript: [
        line({ id: "iv-1", text: "第一个问题" }),
        line({ id: "me-1", speaker: "me", text: "好的" }),
        line({ id: "iv-2", text: "介绍一下你最近负责的项目", ts: 2000 }),
      ],
    });
    render(<MiniView />);

    expect(screen.getByText("介绍一下你最近负责的项目…")).toBeTruthy();
    expect(screen.queryByText("第一个问题…")).toBeNull();
  });

  it("「我」通道的转写不进入迷你页", () => {
    resetStore({
      partial: line({ id: "partial", speaker: "me", text: "我先讲一下我的项目", isFinal: false }),
      transcript: [line({ id: "me-1", speaker: "me", text: "好的" })],
    });
    render(<MiniView />);

    expect(screen.queryByText(/我先讲一下我的项目/)).toBeNull();
    expect(screen.queryByText("好的…")).toBeNull();
    expect(screen.getByText(/等待识别面试官问题/)).toBeTruthy();
  });

  it("已识别出问题时，卡片内保留一行面试官实时文字", () => {
    resetStore({
      currentQuestion: question,
      currentQaId: "q1",
      partial: line({ id: "partial", text: "那再讲讲", isFinal: false }),
    });
    render(<MiniView />);

    expect(screen.getByText("介绍一下你最近负责的项目")).toBeTruthy();
    expect(screen.getByText("那再讲讲…")).toBeTruthy();
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

  it("识别到新问题时自动召回并生成回答（与全屏一致，无需回到完整界面）", async () => {
    const generateAnswer = vi.fn().mockResolvedValue(undefined);
    const recallSources = vi.fn().mockResolvedValue(undefined);
    resetStore({
      asrStatus: runningStatus,
      currentQuestion: question,
      currentQaId: "q1",
      generateAnswer,
      recallSources,
    });
    render(<MiniView />);

    await waitFor(() => expect(generateAnswer).toHaveBeenCalledWith("s-1", "q1"));
    expect(recallSources).toHaveBeenCalledWith("s-1", "q1");
  });

  it("关闭「自动回答」时不触发生成", async () => {
    const generateAnswer = vi.fn().mockResolvedValue(undefined);
    resetStore({
      asrStatus: runningStatus,
      currentQuestion: question,
      currentQaId: "q1",
      questionConfig: autoAnswerOff,
      generateAnswer,
    });
    render(<MiniView />);

    await waitFor(() => expect(useInterviewStore.getState().questionConfig).toBeTruthy());
    expect(generateAnswer).not.toHaveBeenCalled();
  });

  it("无实时召回结果时用落库引用兜底展示", () => {
    resetStore({
      currentQuestion: question,
      sources: [],
      persistedSources: [{ noteId: "note-9", title: "缓存击穿" }],
    });
    render(<MiniView />);

    expect(screen.getByText("缓存击穿")).toBeTruthy();
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

  it("「恢复」按钮退出迷你模式回到完整界面", () => {
    render(<MiniView />);
    fireEvent.click(screen.getByTitle("恢复完整界面"));
    expect(exitMiniMode).toHaveBeenCalledTimes(1);
  });

  it("「退出应用」按钮关闭当前窗口", () => {
    render(<MiniView />);
    fireEvent.click(screen.getByTitle("退出应用"));
    expect(closeWindow).toHaveBeenCalledTimes(1);
  });

  it("迷你页不再提供复制与重新生成按钮（精简为恢复/退出应用）", () => {
    render(<MiniView />);
    expect(screen.queryByTitle("复制回答")).toBeNull();
    expect(screen.queryByTitle("重新生成回答")).toBeNull();
  });
});
