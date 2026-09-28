import { afterEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import {
  FEEDBACK_DISMISS_MS,
  SaveFeedback,
  SaveFeedbackView,
} from "../SaveFeedback";
import { emptySaveFeedback, useSettingsStore } from "../../../stores/settingsStore";

const resetStore = () =>
  useSettingsStore.setState({ saveFeedback: emptySaveFeedback(), error: null });

describe("SaveFeedback（store 连接形态）", () => {
  afterEach(() => {
    act(() => resetStore());
    vi.useRealTimers();
  });

  it("无反馈时不渲染任何内容", () => {
    const { container } = render(<SaveFeedback target="ai" />);
    expect(container.firstChild).toBeNull();
  });

  it("成功反馈渲染后约 3 秒自动消失", () => {
    vi.useFakeTimers();
    render(<SaveFeedback target="ai" />);

    act(() => {
      useSettingsStore
        .getState()
        .setSaveFeedback("ai", { kind: "success", text: "AI 配置已保存" });
    });

    expect(screen.getByText("AI 配置已保存")).toBeTruthy();
    expect(screen.getByRole("status")).toBeTruthy();

    act(() => {
      vi.advanceTimersByTime(FEEDBACK_DISMISS_MS);
    });

    expect(screen.queryByText("AI 配置已保存")).toBeNull();
    expect(useSettingsStore.getState().saveFeedback.ai).toBeNull();
  });

  it("失败反馈常驻（不自动消失），点击关闭后移除", () => {
    vi.useFakeTimers();
    render(<SaveFeedback target="asr" />);

    act(() => {
      useSettingsStore
        .getState()
        .setSaveFeedback("asr", { kind: "error", text: "保存 ASR 配置失败：boom" });
    });

    expect(screen.getByRole("alert")).toBeTruthy();

    act(() => {
      vi.advanceTimersByTime(FEEDBACK_DISMISS_MS * 3);
    });
    expect(screen.getByRole("alert")).toBeTruthy();

    fireEvent.click(screen.getByLabelText("关闭提示"));
    expect(screen.queryByRole("alert")).toBeNull();
    expect(useSettingsStore.getState().saveFeedback.asr).toBeNull();
  });

  it("连续操作时后一次结果覆盖前一次并重置定时器", () => {
    vi.useFakeTimers();
    render(<SaveFeedback target="audio" />);

    act(() => {
      useSettingsStore
        .getState()
        .setSaveFeedback("audio", { kind: "success", text: "音频配置已保存" });
    });
    act(() => {
      vi.advanceTimersByTime(FEEDBACK_DISMISS_MS - 500);
    });

    act(() => {
      useSettingsStore
        .getState()
        .setSaveFeedback("audio", { kind: "success", text: "设备列表已刷新" });
    });
    act(() => {
      vi.advanceTimersByTime(500);
    });
    // 旧定时器已被清理：新反馈仍在
    expect(screen.getByText("设备列表已刷新")).toBeTruthy();

    act(() => {
      vi.advanceTimersByTime(FEEDBACK_DISMISS_MS);
    });
    expect(screen.queryByText("设备列表已刷新")).toBeNull();
  });

  it("只渲染自己分区的反馈", () => {
    render(<SaveFeedback target="knowledge" />);

    act(() => {
      useSettingsStore
        .getState()
        .setSaveFeedback("privacy", { kind: "success", text: "隐私配置已保存" });
    });

    expect(screen.queryByText("隐私配置已保存")).toBeNull();
  });
});

describe("SaveFeedbackView（受控形态，供 UpdateSection 等复用）", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("成功反馈在 3 秒后触发 onDismiss", () => {
    vi.useFakeTimers();
    const onDismiss = vi.fn();
    render(
      <SaveFeedbackView
        feedback={{ kind: "success", text: "更新地址已保存" }}
        onDismiss={onDismiss}
      />
    );

    expect(screen.getByText("更新地址已保存")).toBeTruthy();
    expect(onDismiss).not.toHaveBeenCalled();

    act(() => {
      vi.advanceTimersByTime(FEEDBACK_DISMISS_MS);
    });

    expect(onDismiss).toHaveBeenCalledTimes(1);
  });

  it("失败反馈不触发自动关闭，长文案保留全文便于悬停查看", () => {
    vi.useFakeTimers();
    const onDismiss = vi.fn();
    const text = `保存更新地址失败：${"超长错误信息".repeat(20)}`;
    render(<SaveFeedbackView feedback={{ kind: "error", text }} onDismiss={onDismiss} />);

    act(() => {
      vi.advanceTimersByTime(FEEDBACK_DISMISS_MS * 2);
    });

    expect(onDismiss).not.toHaveBeenCalled();
    expect(screen.getByRole("alert").textContent).toContain("保存更新地址失败");
    expect(screen.getByTitle(text)).toBeTruthy();
  });

  it("feedback 为 null 时不渲染", () => {
    const { container } = render(
      <SaveFeedbackView feedback={null} onDismiss={() => {}} />
    );
    expect(container.firstChild).toBeNull();
  });
});
