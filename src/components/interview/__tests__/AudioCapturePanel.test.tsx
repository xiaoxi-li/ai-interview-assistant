import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import AudioCapturePanel from "../AudioCapturePanel";
import { useInterviewStore } from "../../../stores/interviewStore";
import type { AudioDevice, CaptureStatus } from "../../../types";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

const devices: AudioDevice[] = [
  { id: "mic-usb", name: "麦克风 (USB)", kind: "input", isDefault: true },
  { id: "mix-realtek", name: "立体声混音 (Realtek)", kind: "input", isDefault: false },
];

const status: CaptureStatus = {
  running: false,
  source: null,
  micDevice: "",
  systemDevice: "",
  loopbackAvailable: true,
};

function resetStore(overrides: Partial<ReturnType<typeof useInterviewStore.getState>> = {}) {
  useInterviewStore.setState({
    status,
    inputDevices: devices,
    level: 0,
    error: null,
    starting: false,
    loadCaptureStatus: vi.fn().mockResolvedValue(undefined),
    startCapture: vi.fn().mockResolvedValue(undefined),
    stopCapture: vi.fn().mockResolvedValue(undefined),
    applyLevel: vi.fn(),
    ...overrides,
  });
}

describe("AudioCapturePanel", () => {
  beforeEach(() => {
    resetStore();
  });

  it("展示两种采集来源与设备下拉", () => {
    render(<AudioCapturePanel />);
    expect(screen.getByText("系统音频（面试官）")).toBeTruthy();
    expect(screen.getByText("麦克风（我）")).toBeTruthy();

    const select = screen.getByLabelText("采集设备") as HTMLSelectElement;
    expect(Array.from(select.options).map((o) => o.textContent)).toEqual([
      "使用系统默认设备",
      "麦克风 (USB)（系统默认）",
      "立体声混音 (Realtek)",
    ]);
  });

  it("未采集时显示未开始并可启动", () => {
    const startCapture = vi.fn().mockResolvedValue(undefined);
    resetStore({ startCapture });

    render(<AudioCapturePanel />);
    expect(screen.getByText("未开始")).toBeTruthy();

    fireEvent.click(screen.getByText("开始采集"));
    expect(startCapture).toHaveBeenCalledWith("system", null);
  });

  it("采集中显示状态并可停止", () => {
    const stopCapture = vi.fn().mockResolvedValue(undefined);
    resetStore({
      stopCapture,
      status: { ...status, running: true, source: "system" },
    });

    render(<AudioCapturePanel />);
    expect(screen.getByText("采集中")).toBeTruthy();

    fireEvent.click(screen.getByText("停止采集"));
    expect(stopCapture).toHaveBeenCalledTimes(1);
  });

  it("采集中禁用来源切换与设备选择", () => {
    resetStore({ status: { ...status, running: true, source: "mic" } });
    render(<AudioCapturePanel />);

    const deviceSelect = screen.getByLabelText("采集设备") as HTMLSelectElement;
    expect(deviceSelect.disabled).toBe(true);
    expect(
      (screen.getByText("系统音频（面试官）").closest("button") as HTMLButtonElement)
        .disabled
    ).toBe(true);
  });

  it("选择麦克风后启动会带上设备 ID", () => {
    const startCapture = vi.fn().mockResolvedValue(undefined);
    resetStore({ startCapture });

    render(<AudioCapturePanel />);
    fireEvent.click(screen.getByText("麦克风（我）"));
    fireEvent.change(screen.getByLabelText("采集设备"), {
      target: { value: "mic-usb" },
    });
    fireEvent.click(screen.getByText("开始采集"));

    expect(startCapture).toHaveBeenCalledWith("mic", "mic-usb");
  });

  it("缺少回环设备时在系统音频来源下给出提示", () => {
    resetStore({
      status: { ...status, loopbackAvailable: false },
    });
    render(<AudioCapturePanel />);

    expect(screen.getByText(/未检测到系统音频回环设备/)).toBeTruthy();

    fireEvent.click(screen.getByText("麦克风（我）"));
    expect(screen.queryByText(/未检测到系统音频回环设备/)).toBeNull();
  });

  it("电平百分比随 store 更新", () => {
    resetStore({ level: 0.42 });
    render(<AudioCapturePanel />);
    expect(screen.getByText("42%")).toBeTruthy();
  });

  it("展示错误信息", () => {
    resetStore({ error: "未找到输入设备：abc" });
    render(<AudioCapturePanel />);
    expect(screen.getByText("未找到输入设备：abc")).toBeTruthy();
  });
});
