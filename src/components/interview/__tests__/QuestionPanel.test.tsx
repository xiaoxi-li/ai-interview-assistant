import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import QuestionPanel from "../QuestionPanel";
import { useInterviewStore } from "../../../stores/interviewStore";
import type { AsrSessionStatus, DetectedQuestion, QaPair } from "../../../types";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

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

const idleStatus: AsrSessionStatus = { ...runningStatus, running: false };

const question: DetectedQuestion = {
  text: "介绍一下你最近负责的项目",
  score: 1.05,
  signals: ["祈使式提问「介绍一下」", "第二人称疑问「你的」"],
  ts: Date.now(),
};

function qaPair(overrides: Partial<QaPair> = {}): QaPair {
  return {
    id: "q1",
    sessionId: "s-1",
    question: "介绍一下你最近负责的项目",
    answer: "",
    sources: [],
    ts: Date.now(),
    ...overrides,
  };
}

function resetStore(overrides: Partial<ReturnType<typeof useInterviewStore.getState>> = {}) {
  useInterviewStore.setState({
    asrStatus: runningStatus,
    currentQuestion: null,
    questionManual: false,
    questions: [],
    currentQaId: null,
    sources: [],
    sourcesLoading: false,
    recallQueries: [],
    triggerQuestion: vi.fn().mockResolvedValue(undefined),
    loadQuestions: vi.fn().mockResolvedValue(undefined),
    applyQuestionPush: vi.fn(),
    ...overrides,
  });
}

describe("QuestionPanel", () => {
  beforeEach(() => {
    resetStore();
  });

  it("未识别到问题时按转写状态给出引导", () => {
    render(<QuestionPanel />);
    expect(screen.getByText(/正在监听面试官语音/)).toBeTruthy();
  });

  it("未开始转写时提示可手动触发", () => {
    resetStore({ asrStatus: idleStatus });
    render(<QuestionPanel />);
    expect(screen.getByText(/也可随时手动触发识别/)).toBeTruthy();
  });

  it("展示问题文本、置信度与命中信号", () => {
    resetStore({ currentQuestion: question, questions: [qaPair()] });
    render(<QuestionPanel />);

    expect(screen.getByText("介绍一下你最近负责的项目")).toBeTruthy();
    expect(screen.getByText("置信度 1.05")).toBeTruthy();
    expect(screen.getByText("祈使式提问「介绍一下」")).toBeTruthy();
    expect(screen.getByText("第二人称疑问「你的」")).toBeTruthy();
    expect(screen.getByText("已识别 1 个问题")).toBeTruthy();
  });

  it("区分自动识别与手动触发", () => {
    resetStore({ currentQuestion: question, questionManual: false });
    const { rerender } = render(<QuestionPanel />);
    expect(screen.getByText("自动识别")).toBeTruthy();

    resetStore({ currentQuestion: question, questionManual: true });
    rerender(<QuestionPanel />);
    expect(screen.getByText("手动触发")).toBeTruthy();
  });

  it("点击手动识别触发 store 动作", () => {
    const triggerQuestion = vi.fn().mockResolvedValue(undefined);
    resetStore({ triggerQuestion });
    render(<QuestionPanel />);

    fireEvent.click(screen.getByText("手动识别当前问题"));
    expect(triggerQuestion).toHaveBeenCalledTimes(1);
  });

  it("未转写时禁用手动识别", () => {
    resetStore({ asrStatus: idleStatus });
    render(<QuestionPanel />);

    const button = screen.getByText("手动识别当前问题").closest("button") as HTMLButtonElement;
    expect(button.disabled).toBe(true);
  });

  it("有会话时加载历史问题", () => {
    const loadQuestions = vi.fn().mockResolvedValue(undefined);
    resetStore({ loadQuestions });
    render(<QuestionPanel />);

    expect(loadQuestions).toHaveBeenCalledWith("s-1");
  });

  it("展示近期识别的问题列表", () => {
    resetStore({
      questions: [
        qaPair({ id: "q1", question: "第一个问题" }),
        qaPair({ id: "q2", question: "第二个问题" }),
      ],
    });
    render(<QuestionPanel />);

    expect(screen.getByText("第一个问题")).toBeTruthy();
    expect(screen.getByText("第二个问题")).toBeTruthy();
  });

  it("不再展示知识库召回相关内容（IA-07 变更，统一由引用来源呈现）", () => {
    resetStore({
      currentQuestion: question,
      currentQaId: "q1",
      sources: [
        {
          noteId: "note-1",
          title: "项目架构设计",
          snippet: "负责桌面应用的架构设计与性能优化",
          score: 0.0328,
        },
      ],
    });
    render(<QuestionPanel />);

    expect(screen.queryByText("知识库参考")).toBeNull();
    expect(screen.queryByText("项目架构设计")).toBeNull();
    expect(screen.queryByText(/条片段/)).toBeNull();
    expect(screen.queryByText(/未召回到相关内容/)).toBeNull();
  });
});
