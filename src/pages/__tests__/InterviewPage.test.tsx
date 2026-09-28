import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import InterviewPage from "../InterviewPage";
import { useAppStore } from "../../stores/appStore";
import { useInterviewStore } from "../../stores/interviewStore";
import type { AsrSessionStatus } from "../../types";

// 布局用例只关心「哪个面板挂在哪一列」，子面板一律桩化为带 testid 的占位元素
vi.mock("../../components/interview/DemoCard", () => ({
  default: () => <div data-testid="demo-card" />,
}));
vi.mock("../../components/interview/AudioCapturePanel", () => ({
  default: () => <div data-testid="audio-capture-panel" />,
}));
vi.mock("../../components/interview/TranscriptPanel", () => ({
  default: () => <div data-testid="transcript-panel" />,
}));
vi.mock("../../components/interview/ShortcutBar", () => ({
  default: () => <div data-testid="shortcut-bar" />,
}));
vi.mock("../../components/interview/QuestionPanel", () => ({
  default: () => <div data-testid="question-panel" />,
}));
vi.mock("../../components/interview/AnswerPanel", () => ({
  default: () => <div data-testid="answer-panel" />,
}));

const idleStatus: AsrSessionStatus = {
  running: false,
  sessionId: null,
  source: null,
  speaker: null,
  engine: null,
  startedAt: null,
  paused: false,
  dual: false,
  aecEnabled: false,
  aecErleDb: null,
};

const runningStatus: AsrSessionStatus = { ...idleStatus, running: true, source: "dual", dual: true };

function resetStore(asrStatus: AsrSessionStatus = idleStatus) {
  useInterviewStore.setState({ asrStatus });
}

/** 面板所在列容器（面板 → 列） */
function columnOf(testId: string): HTMLElement {
  return screen.getByTestId(testId).parentElement as HTMLElement;
}

/** 列容器内直接子面板的 testid 顺序 */
function childIds(column: HTMLElement): (string | null)[] {
  return Array.from(column.children).map((child) => child.getAttribute("data-testid"));
}

describe("InterviewPage 布局", () => {
  beforeEach(() => {
    useAppStore.setState({ locale: "zh-CN" });
    resetStore();
  });

  it("左列按 45% 自适应、右列占满剩余（不用固定宽度）", () => {
    render(<InterviewPage />);

    const left = columnOf("transcript-panel");
    const right = columnOf("shortcut-bar");

    expect(left.className).toContain("w-[45%]");
    expect(left.className).toContain("shrink-0");
    expect(right.className).toContain("flex-1");
    expect(right.className).toContain("min-w-0");
  });

  it("两列都遵循「整体滚动 + 子面板不压缩」约定", () => {
    render(<InterviewPage />);

    for (const column of [columnOf("transcript-panel"), columnOf("shortcut-bar")]) {
      expect(column.className).toContain("overflow-y-auto");
      expect(column.className).toContain("[&>*]:shrink-0");
    }
  });

  it("左列自上而下为：演示卡 → 音频采集 → 实时转写", () => {
    render(<InterviewPage />);

    const left = columnOf("transcript-panel");
    expect(childIds(left)).toEqual(["demo-card", "audio-capture-panel", "transcript-panel"]);
  });

  it("右列自上而下为：快捷操作 → 问题识别 → AI 回答", () => {
    render(<InterviewPage />);

    const right = columnOf("shortcut-bar");
    expect(childIds(right)).toEqual(["shortcut-bar", "question-panel", "answer-panel"]);
  });

  it("转写开始后左列不再渲染演示卡，其余归属不变", () => {
    resetStore(runningStatus);
    render(<InterviewPage />);

    expect(screen.queryByTestId("demo-card")).toBeNull();

    const left = columnOf("transcript-panel");
    const right = columnOf("answer-panel");
    expect(childIds(left)).toEqual(["audio-capture-panel", "transcript-panel"]);
    expect(childIds(right)).toEqual(["shortcut-bar", "question-panel", "answer-panel"]);
  });
});
