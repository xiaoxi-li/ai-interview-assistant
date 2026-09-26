import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import AnswerPanel from "../AnswerPanel";
import { useAppStore } from "../../../stores/appStore";
import { useInterviewStore } from "../../../stores/interviewStore";
import { useKnowledgeStore } from "../../../stores/knowledgeStore";
import type {
  AnswerStatus,
  AsrSessionStatus,
  QuestionConfig,
  RecallHit,
} from "../../../types";

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

const questionConfig: QuestionConfig = {
  autoDetect: true,
  threshold: 0.5,
  minChars: 4,
  mergeWindowMs: 3000,
  autoAnswer: true,
};

const hit: RecallHit = {
  noteId: "note-1",
  title: "项目架构设计",
  snippet: "负责桌面应用的架构设计",
  score: 0.03,
};

function resetStore(overrides: Partial<ReturnType<typeof useInterviewStore.getState>> = {}) {
  useInterviewStore.setState({
    asrStatus: runningStatus,
    currentQaId: null,
    questionConfig,
    answerText: "",
    answerStatus: "idle" as AnswerStatus,
    answerError: null,
    answerQaId: null,
    answerModel: null,
    sources: [],
    persistedSources: [],
    generateAnswer: vi.fn().mockResolvedValue(undefined),
    applyAnswerPush: vi.fn(),
    loadQuestionConfig: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  });
}

describe("AnswerPanel", () => {
  beforeEach(() => {
    resetStore();
  });

  it("未识别到问题时给出引导", () => {
    render(<AnswerPanel />);
    expect(screen.getByText(/尚未识别到问题/)).toBeTruthy();
  });

  it("流式输出期间展示已生成内容", () => {
    resetStore({
      currentQaId: "q1",
      answerStatus: "streaming",
      answerText: "我最近负责桌面端应用的架构设计",
    });
    render(<AnswerPanel />);

    expect(screen.getByText(/我最近负责桌面端应用的架构设计/)).toBeTruthy();
    expect(screen.queryByText(/正在结合知识库生成回答/)).toBeNull();
  });

  it("生成中（尚无 token）展示进行中文案", () => {
    resetStore({ currentQaId: "q1", answerStatus: "thinking" });
    render(<AnswerPanel />);
    expect(screen.getByText(/正在结合知识库生成回答/)).toBeTruthy();
  });

  it("完成后展示已完成标记", () => {
    resetStore({
      currentQaId: "q1",
      answerStatus: "done",
      answerText: "完整回答内容",
      answerModel: "deepseek",
    });
    render(<AnswerPanel />);

    expect(screen.getByText("完整回答内容")).toBeTruthy();
    expect(screen.getByText("已完成")).toBeTruthy();
    expect(screen.getByText("deepseek")).toBeTruthy();
  });

  it("失败时展示错误信息", () => {
    resetStore({
      currentQaId: "q1",
      answerStatus: "error",
      answerError: "尚未配置「deepseek」的 API Key，请到「设置 → AI 平台」保存后重试。",
    });
    render(<AnswerPanel />);

    expect(screen.getByText(/尚未配置「deepseek」的 API Key/)).toBeTruthy();
  });

  it("识别到新问题时按配置自动生成回答", async () => {
    const generateAnswer = vi.fn().mockResolvedValue(undefined);
    resetStore({ currentQaId: "q1", generateAnswer });
    render(<AnswerPanel />);

    await waitFor(() => expect(generateAnswer).toHaveBeenCalledWith("s-1", "q1"));
  });

  it("关闭自动回答时不触发生成", async () => {
    const generateAnswer = vi.fn().mockResolvedValue(undefined);
    resetStore({
      currentQaId: "q1",
      generateAnswer,
      questionConfig: { ...questionConfig, autoAnswer: false },
    });
    render(<AnswerPanel />);

    // 等待配置加载后的若干次渲染
    await waitFor(() => expect(useInterviewStore.getState().questionConfig).toBeTruthy());
    expect(generateAnswer).not.toHaveBeenCalled();
  });

  it("点击重新生成传入 regenerate 标记", () => {
    const generateAnswer = vi.fn().mockResolvedValue(undefined);
    resetStore({
      currentQaId: "q1",
      answerText: "旧回答",
      answerStatus: "done",
      generateAnswer,
    });
    render(<AnswerPanel />);

    fireEvent.click(screen.getByText("重新生成"));
    expect(generateAnswer).toHaveBeenCalledWith("s-1", "q1", true);
  });

  it("无问题时禁用重新生成", () => {
    render(<AnswerPanel />);
    const button = screen.getByText("重新生成").closest("button") as HTMLButtonElement;
    expect(button.disabled).toBe(true);
  });

  it("展示引用来源（标题 / 摘要 / 得分）", () => {
    resetStore({
      currentQaId: "q1",
      answerText: "回答",
      answerStatus: "done",
      sources: [hit],
    });
    render(<AnswerPanel />);

    expect(screen.getByText("引用来源")).toBeTruthy();
    expect(screen.getByText("1 篇")).toBeTruthy();
    expect(screen.getByText("项目架构设计")).toBeTruthy();
    expect(screen.getByText("负责桌面应用的架构设计")).toBeTruthy();
    expect(screen.getByText("0.030")).toBeTruthy();
  });

  it("点击引用跳转知识库并选中该笔记", () => {
    const select = vi.fn();
    const setCurrentPage = vi.fn();
    useKnowledgeStore.setState({ select } as Partial<
      ReturnType<typeof useKnowledgeStore.getState>
    >);
    useAppStore.setState({ setCurrentPage });

    resetStore({ currentQaId: "q1", answerText: "回答", answerStatus: "done", sources: [hit] });
    render(<AnswerPanel />);

    fireEvent.click(screen.getByText("项目架构设计"));
    expect(select).toHaveBeenCalledWith("note-1");
    expect(setCurrentPage).toHaveBeenCalledWith("knowledge");
  });

  it("无实时召回结果时回退展示落库引用", () => {
    resetStore({
      currentQaId: "q1",
      answerText: "回答",
      answerStatus: "done",
      sources: [],
      persistedSources: [{ noteId: "n-9", title: "历史引用笔记" }],
    });
    render(<AnswerPanel />);

    expect(screen.getByText("历史引用笔记")).toBeTruthy();
    expect(screen.queryByText("负责桌面应用的架构设计")).toBeNull();
  });

  it("缺少笔记 ID 的引用不可点击", () => {
    const setCurrentPage = vi.fn();
    useAppStore.setState({ setCurrentPage });

    resetStore({
      currentQaId: "q1",
      answerText: "回答",
      answerStatus: "done",
      sources: [],
      persistedSources: [{ noteId: "", title: "旧格式引用" }],
    });
    render(<AnswerPanel />);

    const button = screen.getByText("旧格式引用").closest("button") as HTMLButtonElement;
    expect(button.disabled).toBe(true);
    fireEvent.click(button);
    expect(setCurrentPage).not.toHaveBeenCalled();
  });
});
