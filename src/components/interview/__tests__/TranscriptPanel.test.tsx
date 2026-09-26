import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import TranscriptPanel from "../TranscriptPanel";
import { useAppStore } from "../../../stores/appStore";
import { useInterviewStore, type TranscriptLine } from "../../../stores/interviewStore";
import type { AsrEngineInfo, AsrSessionStatus } from "../../../types";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

const readyEngine: AsrEngineInfo = {
  id: "asr-paraformer-zh",
  label: "本地 ASR（sherpa-onnx 中文 paraformer）",
  ready: true,
  detail: "模型已就绪",
};

const blockedEngine: AsrEngineInfo = {
  ...readyEngine,
  ready: false,
  detail: "本地 ASR 模型尚未下载：C:/models/asr/xxx。请前往「设置 → 模型管理」下载后重试。",
};

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

function finalLine(overrides: Partial<TranscriptLine> = {}): TranscriptLine {
  return {
    id: "t1",
    speaker: "interviewer",
    text: "请介绍一下你最近的项目",
    isFinal: true,
    ts: 1000,
    ...overrides,
  };
}

function resetStore(overrides: Partial<ReturnType<typeof useInterviewStore.getState>> = {}) {
  useInterviewStore.setState({
    asrStatus: idleStatus,
    engines: [readyEngine],
    transcript: [],
    partial: null,
    asrError: null,
    startingAsr: false,
    loadAsrStatus: vi.fn().mockResolvedValue(undefined),
    startTranscription: vi.fn().mockResolvedValue(undefined),
    startDualTranscription: vi.fn().mockResolvedValue(undefined),
    stopTranscription: vi.fn().mockResolvedValue(undefined),
    applyTranscriptPush: vi.fn(),
    clearTranscript: vi.fn(),
    ...overrides,
  });
}

describe("TranscriptPanel", () => {
  beforeEach(() => {
    // 每个用例重置语言，避免相互影响
    useAppStore.setState({ locale: "zh-CN" });
    resetStore();
  });

  it("英文环境下展示英文文案", () => {
    useAppStore.setState({ locale: "en-US" });
    resetStore();
    render(<TranscriptPanel />);

    expect(screen.getByText("Live transcript")).toBeTruthy();
    expect(screen.getByText("Start transcription")).toBeTruthy();
    expect(screen.getByText("Dual channel (speaker separation)")).toBeTruthy();
  });

  it("空状态提示音频不出本机", () => {
    render(<TranscriptPanel />);
    expect(screen.getByText(/音频不出本机/)).toBeTruthy();
  });

  it("展示最终转写与说话人标签", () => {
    resetStore({
      transcript: [
        finalLine(),
        finalLine({ id: "t2", speaker: "me", text: "好的，我介绍一下" }),
      ],
    });
    render(<TranscriptPanel />);

    expect(screen.getByText("请介绍一下你最近的项目")).toBeTruthy();
    expect(screen.getByText("好的，我介绍一下")).toBeTruthy();
    expect(screen.getByText("面试官")).toBeTruthy();
    expect(screen.getByText("我")).toBeTruthy();
  });

  it("进行中的 partial 以省略号样式展示", () => {
    resetStore({
      transcript: [finalLine()],
      partial: finalLine({ id: "partial", text: "然后我负责", isFinal: false }),
    });
    render(<TranscriptPanel />);

    expect(screen.getByText("然后我负责…")).toBeTruthy();
  });

  it("未就绪引擎给出可操作提示", () => {
    resetStore({ engines: [blockedEngine] });
    render(<TranscriptPanel />);
    expect(screen.getByText(/请前往「设置 → 模型管理」下载/)).toBeTruthy();
  });

  it("转写中展示状态并隐藏渠道切换", () => {
    resetStore({
      asrStatus: { ...idleStatus, running: true, source: "system", engine: "sherpa-onnx" },
    });
    render(<TranscriptPanel />);

    expect(screen.getByText("转写中")).toBeTruthy();
    expect(screen.getByText("sherpa-onnx")).toBeTruthy();
    expect(
      (screen.getByText("我（麦克风）").closest("button") as HTMLButtonElement).disabled
    ).toBe(true);
  });

  it("默认双通道模式，点击开始调用双通道启动", () => {
    const startDualTranscription = vi.fn().mockResolvedValue(undefined);
    resetStore({ startDualTranscription });
    render(<TranscriptPanel />);

    fireEvent.click(screen.getByText("开始转写"));
    expect(startDualTranscription).toHaveBeenCalledWith(null, null);
  });

  it("切换到单通道后按来源启动", () => {
    const startTranscription = vi.fn().mockResolvedValue(undefined);
    resetStore({ startTranscription });
    render(<TranscriptPanel />);

    fireEvent.click(screen.getByText("我（麦克风）"));
    fireEvent.click(screen.getByText("开始转写"));
    expect(startTranscription).toHaveBeenCalledWith("mic", null);
  });

  it("双通道会话展示状态与回声抑制量", () => {
    resetStore({
      asrStatus: {
        ...idleStatus,
        running: true,
        source: "dual",
        dual: true,
        engine: "sherpa-onnx",
        aecEnabled: true,
        aecErleDb: 21.4,
      },
    });
    render(<TranscriptPanel />);

    expect(screen.getByText("双通道转写中")).toBeTruthy();
    expect(screen.getByText(/回声抑制 21\.4dB/)).toBeTruthy();
  });

  it("转写中点击按钮触发停止", () => {
    const stopTranscription = vi.fn().mockResolvedValue(undefined);
    resetStore({
      stopTranscription,
      asrStatus: { ...idleStatus, running: true, source: "system" },
    });
    render(<TranscriptPanel />);

    fireEvent.click(screen.getByText("停止转写"));
    expect(stopTranscription).toHaveBeenCalledTimes(1);
  });

  it("展示转写错误", () => {
    resetStore({
      asrError:
        "云端 ASR 尚未接入（计划于 P1）。请到「设置 → 语音识别」将模式切换为「本地（隐私优先）」后再开始转写。",
    });
    render(<TranscriptPanel />);
    expect(screen.getByText(/云端 ASR 尚未接入/)).toBeTruthy();
  });

  it("清空按钮触发 store 动作", () => {
    const clearTranscript = vi.fn();
    resetStore({ clearTranscript, transcript: [finalLine()] });
    render(<TranscriptPanel />);

    fireEvent.click(screen.getByTitle("清空当前转写显示"));
    expect(clearTranscript).toHaveBeenCalledTimes(1);
  });
});
