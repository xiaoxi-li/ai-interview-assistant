import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import ShortcutBar from "../ShortcutBar";
import { useInterviewStore } from "../../../stores/interviewStore";
import { useSettingsStore } from "../../../stores/settingsStore";
import type { AsrSessionStatus, InterfaceConfig } from "../../../types";

/** 记录各事件监听器，便于测试中模拟后端推送 */
const listeners: Record<string, (event: { payload: unknown }) => void> = {};

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((event: string, handler: (payload: { payload: unknown }) => void) => {
    listeners[event] = handler;
    return Promise.resolve(() => {
      delete listeners[event];
    });
  }),
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

const interfaceConfig: InterfaceConfig = {
  theme: "dark",
  hotkeys: [],
  autoStart: false,
  trayBehavior: "minimize",
  alwaysOnTop: false,
  hideFromCapture: false,
};

const clipboardWrite = vi.fn().mockResolvedValue(undefined);

function resetStore(overrides: Partial<ReturnType<typeof useInterviewStore.getState>> = {}) {
  useInterviewStore.setState({
    asrStatus: runningStatus,
    currentQaId: "q1",
    answerText: "已生成的回答",
    answerStatus: "done",
    triggerQuestion: vi.fn().mockResolvedValue(undefined),
    setAsrPaused: vi.fn().mockResolvedValue(undefined),
    generateAnswer: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  });

  useSettingsStore.setState({
    interfaceConfig,
    loadInterfaceSettings: vi.fn().mockResolvedValue(undefined),
    setStealthWindow: vi.fn().mockResolvedValue(undefined),
  } as Partial<ReturnType<typeof useSettingsStore.getState>>);
}

function pressShortcut(key: string, target?: HTMLElement) {
  (target ?? window).dispatchEvent(
    new KeyboardEvent("keydown", {
      key,
      ctrlKey: true,
      shiftKey: true,
      bubbles: true,
      cancelable: true,
    })
  );
}

describe("ShortcutBar", () => {
  beforeEach(() => {
    resetStore();
    // 让 isTauri() 判定为真，从而挂载全局事件监听
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
    delete listeners["shortcut://action"];
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText: clipboardWrite },
    });
    clipboardWrite.mockClear();
  });

  it("未开始转写时禁用提问与暂停", () => {
    resetStore({ asrStatus: idleStatus });
    render(<ShortcutBar />);

    expect((screen.getByText("一键提问").closest("button") as HTMLButtonElement).disabled).toBe(
      true
    );
    expect((screen.getByText("暂停采集").closest("button") as HTMLButtonElement).disabled).toBe(
      true
    );
  });

  it("点击一键提问手动触发识别", () => {
    const triggerQuestion = vi.fn().mockResolvedValue(undefined);
    resetStore({ triggerQuestion });
    render(<ShortcutBar />);

    fireEvent.click(screen.getByText("一键提问"));
    expect(triggerQuestion).toHaveBeenCalledTimes(1);
  });

  it("暂停与继续采集切换", () => {
    const setAsrPaused = vi.fn().mockResolvedValue(undefined);
    resetStore({ setAsrPaused });
    const { rerender } = render(<ShortcutBar />);

    fireEvent.click(screen.getByText("暂停采集"));
    expect(setAsrPaused).toHaveBeenLastCalledWith(true);

    resetStore({ setAsrPaused, asrStatus: { ...runningStatus, paused: true } });
    rerender(<ShortcutBar />);
    fireEvent.click(screen.getByText("继续采集"));
    expect(setAsrPaused).toHaveBeenLastCalledWith(false);
  });

  it("无问题或无回答时对应按钮禁用", () => {
    resetStore({ currentQaId: null, answerText: "" });
    render(<ShortcutBar />);

    expect(
      (screen.getByText("重新生成").closest("button") as HTMLButtonElement).disabled
    ).toBe(true);
    expect((screen.getByText("复制回答").closest("button") as HTMLButtonElement).disabled).toBe(
      true
    );
  });

  it("点击重新生成传入 regenerate 标记", () => {
    const generateAnswer = vi.fn().mockResolvedValue(undefined);
    resetStore({ generateAnswer });
    render(<ShortcutBar />);

    fireEvent.click(screen.getByText("重新生成"));
    expect(generateAnswer).toHaveBeenCalledWith("s-1", "q1", true);
  });

  it("点击复制回答写入剪贴板", () => {
    render(<ShortcutBar />);
    fireEvent.click(screen.getByText("复制回答"));
    expect(clipboardWrite).toHaveBeenCalledWith("已生成的回答");
  });

  it("快捷键 Ctrl+Shift+A 触发一键提问", () => {
    const triggerQuestion = vi.fn().mockResolvedValue(undefined);
    resetStore({ triggerQuestion });
    render(<ShortcutBar />);

    pressShortcut("a");
    expect(triggerQuestion).toHaveBeenCalledTimes(1);
  });

  it("快捷键 Ctrl+Shift+P 暂停采集", () => {
    const setAsrPaused = vi.fn().mockResolvedValue(undefined);
    resetStore({ setAsrPaused });
    render(<ShortcutBar />);

    pressShortcut("p");
    expect(setAsrPaused).toHaveBeenCalledWith(true);
  });

  it("快捷键 Ctrl+Shift+R 重新生成回答", () => {
    const generateAnswer = vi.fn().mockResolvedValue(undefined);
    resetStore({ generateAnswer });
    render(<ShortcutBar />);

    pressShortcut("r");
    expect(generateAnswer).toHaveBeenCalledWith("s-1", "q1", true);
  });

  it("快捷键 Ctrl+Shift+C 复制回答", () => {
    render(<ShortcutBar />);
    pressShortcut("c");
    expect(clipboardWrite).toHaveBeenCalledWith("已生成的回答");
  });

  it("在输入框中不触发快捷键", () => {
    const triggerQuestion = vi.fn().mockResolvedValue(undefined);
    resetStore({ triggerQuestion });
    const { container } = render(<ShortcutBar />);

    const input = document.createElement("input");
    container.appendChild(input);
    pressShortcut("a", input);

    expect(triggerQuestion).not.toHaveBeenCalled();
  });

  it("响应系统全局热键推送", async () => {
    const triggerQuestion = vi.fn().mockResolvedValue(undefined);
    resetStore({ triggerQuestion });
    render(<ShortcutBar />);

    // 等待 listen 完成注册
    await vi.waitFor(() => expect(listeners["shortcut://action"]).toBeTruthy());
    listeners["shortcut://action"]({ payload: { action: "ask" } });

    expect(triggerQuestion).toHaveBeenCalledTimes(1);
  });

  it("展示快捷键提示", () => {
    render(<ShortcutBar />);
    expect(screen.getByText(/Ctrl\+Shift\+A/)).toBeTruthy();
  });

  it("切换窗口置顶", () => {
    render(<ShortcutBar />);
    fireEvent.click(screen.getByTitle("隐身模式：窗口置顶"));
    expect(useSettingsStore.getState().setStealthWindow).toHaveBeenCalledWith({
      alwaysOnTop: true,
    });
  });

  it("切换防录屏并展示激活状态", () => {
    useSettingsStore.setState({
      interfaceConfig: { ...interfaceConfig, hideFromCapture: true },
    } as Partial<ReturnType<typeof useSettingsStore.getState>>);
    render(<ShortcutBar />);

    const button = screen.getByTitle("隐身模式：屏幕共享时隐藏窗口内容（防录屏）");
    expect(button.getAttribute("aria-pressed")).toBe("true");

    fireEvent.click(button);
    expect(useSettingsStore.getState().setStealthWindow).toHaveBeenCalledWith({
      hideFromCapture: false,
    });
  });
});
