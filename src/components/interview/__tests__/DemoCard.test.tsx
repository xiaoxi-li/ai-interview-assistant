import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import DemoCard from "../DemoCard";
import { useAppStore } from "../../../stores/appStore";
import { useInterviewStore } from "../../../stores/interviewStore";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

function resetStore(overrides: Partial<ReturnType<typeof useInterviewStore.getState>> = {}) {
  useInterviewStore.setState({
    startingAsr: false,
    asrError: null,
    demoOfflineAnswer: true,
    startDemoSession: vi.fn().mockResolvedValue(undefined),
    stopTranscription: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  } as Partial<ReturnType<typeof useInterviewStore.getState>>);
}

describe("DemoCard", () => {
  beforeEach(() => {
    useAppStore.setState({ locale: "zh-CN" });
    resetStore();
  });

  it("展示演示说明与链路步骤", () => {
    render(<DemoCard />);

    expect(screen.getByText("一键演示 · 完整链路")).toBeTruthy();
    expect(screen.getByText(/无需麦克风、模型或 API Key/)).toBeTruthy();
    expect(screen.getByText("实时转写（面试官 / 我）")).toBeTruthy();
    expect(screen.getByText("AI 流式回答 + 引用来源")).toBeTruthy();
    expect(screen.getByText(/演示会自动写入 4 篇示例笔记/)).toBeTruthy();
  });

  it("未配置 AI Key 时提示将使用离线回答", () => {
    render(<DemoCard />);
    expect(screen.getByText(/未配置 AI Key，将使用离线拼装回答/)).toBeTruthy();
  });

  it("点击开始演示触发一键演示", () => {
    const startDemoSession = vi.fn().mockResolvedValue(undefined);
    resetStore({ startDemoSession });
    render(<DemoCard />);

    fireEvent.click(screen.getByText("开始演示"));
    expect(startDemoSession).toHaveBeenCalledTimes(1);
  });

  it("演示启动中禁用按钮并提示准备数据", () => {
    resetStore({ startingAsr: true });
    render(<DemoCard />);

    const button = screen.getByText("开始演示").closest("button") as HTMLButtonElement;
    expect(button.disabled).toBe(true);
    expect(screen.getByText(/正在准备演示数据/)).toBeTruthy();
  });

  it("展示启动失败原因", () => {
    resetStore({ asrError: "演示启动失败：数据库忙" });
    render(<DemoCard />);
    expect(screen.getByText(/演示启动失败/)).toBeTruthy();
  });

  it("英文环境下展示英文文案", () => {
    useAppStore.setState({ locale: "en-US" });
    resetStore();
    render(<DemoCard />);

    expect(screen.getByText("One-click demo · full pipeline")).toBeTruthy();
    expect(screen.getByText("Start demo")).toBeTruthy();
  });
});
