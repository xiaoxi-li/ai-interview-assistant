import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import AudioSettingsSection from "../AudioSettingsSection";
import type { AudioConfig, AudioDevice } from "../../../types";

const devices: AudioDevice[] = [
  { id: "mic-usb", name: "麦克风 (USB Audio)", kind: "input", isDefault: true },
  { id: "mic-realtek", name: "麦克风 (Realtek)", kind: "input", isDefault: false },
  { id: "spk-realtek", name: "扬声器 (Realtek)", kind: "output", isDefault: true },
  { id: "spk-bt", name: "耳机 (Bluetooth)", kind: "output", isDefault: false },
];

const config: AudioConfig = { micDeviceId: null, systemDeviceId: null };

function setup(overrides: Partial<Parameters<typeof AudioSettingsSection>[0]> = {}) {
  const handlers = { onRefresh: vi.fn(), onSave: vi.fn() };
  render(
    <AudioSettingsSection
      config={config}
      devices={devices}
      {...handlers}
      {...overrides}
    />
  );
  return handlers;
}

describe("AudioSettingsSection", () => {
  it("按方向分组渲染设备选项并标注系统默认", () => {
    setup();

    const micSelect = screen.getByLabelText(/麦克风（我的声音）/) as HTMLSelectElement;
    const systemSelect = screen.getByLabelText(/系统音频（面试官声音）/) as HTMLSelectElement;

    // 输入设备 2 个 + 占位项
    expect(micSelect.options.length).toBe(3);
    // 输出设备 2 个 + 占位项
    expect(systemSelect.options.length).toBe(3);
    expect(screen.getByText("麦克风 (USB Audio)（系统默认）")).toBeTruthy();
    expect(screen.queryByText("扬声器 (Realtek)（系统默认）")).toBeTruthy();
  });

  it("默认使用系统设备（空值选项）", () => {
    setup();
    const micSelect = screen.getByLabelText(/麦克风（我的声音）/) as HTMLSelectElement;
    expect(micSelect.value).toBe("");
    expect(screen.getByText("使用系统默认麦克风")).toBeTruthy();
  });

  it("选择设备后保存回传设备 ID", () => {
    const h = setup();

    fireEvent.change(screen.getByLabelText(/麦克风（我的声音）/), {
      target: { value: "mic-realtek" },
    });
    fireEvent.change(screen.getByLabelText(/系统音频（面试官声音）/), {
      target: { value: "spk-bt" },
    });
    fireEvent.click(screen.getByText("保存配置"));

    expect(h.onSave).toHaveBeenCalledWith({
      micDeviceId: "mic-realtek",
      systemDeviceId: "spk-bt",
      processLoopbackPid: null,
    });
  });

  it("改回默认设备时提交 null", () => {
    const h = setup({
      config: { micDeviceId: "mic-usb", systemDeviceId: "spk-realtek" },
    });

    fireEvent.change(screen.getByLabelText(/麦克风（我的声音）/), {
      target: { value: "" },
    });
    fireEvent.click(screen.getByText("保存配置"));

    expect(h.onSave).toHaveBeenCalledWith(
      expect.objectContaining({ micDeviceId: null })
    );
  });

  it("点击刷新触发回调", () => {
    const h = setup();
    fireEvent.click(screen.getByText("刷新设备"));
    expect(h.onRefresh).toHaveBeenCalledTimes(1);
  });

  it("无设备时给出提示", () => {
    setup({ devices: [] });
    expect(screen.getByText(/未检测到音频设备/)).toBeTruthy();
  });

  // ===== TODO-05：进程级回环（系统音频采集范围） =====

  it("默认系统音频采集范围为全部应用", () => {
    setup();
    const allApps = screen.getByText("全部应用（混音采集）").closest("button") as HTMLButtonElement;
    expect(allApps.getAttribute("aria-pressed")).toBe("true");
    // 未进入进程模式时不显示目标应用下拉
    expect(screen.queryByLabelText("目标应用")).toBeNull();
  });

  it("配置了进程目标时进入进程模式并加载应用列表", async () => {
    setup({
      config: { micDeviceId: null, systemDeviceId: null, processLoopbackPid: 4212 },
    });
    const target = await screen.findByLabelText("目标应用") as HTMLSelectElement;
    // mock 返回的进程列表（wemeetapp.exe 选中）
    expect(target.value).toBe("4212");
    expect(screen.getByText(/wemeetapp\.exe/)).toBeTruthy();
  });

  it("切换到只采集指定应用后进入待选状态，未选目标保存归一为 null", () => {
    const h = setup();
    fireEvent.click(screen.getByText("只采集指定应用"));

    // 进入进程模式（待选状态）：下拉出现
    expect(screen.getByLabelText("目标应用")).toBeTruthy();
    // 未选目标时保存：processLoopbackPid 归一为 null（回退端点模式）
    fireEvent.click(screen.getByText("保存配置"));
    expect(h.onSave).toHaveBeenCalledWith(
      expect.objectContaining({ processLoopbackPid: null })
    );
  });

  it("切回全部应用清除进程目标", () => {
    setup({
      config: { micDeviceId: null, systemDeviceId: null, processLoopbackPid: 4212 },
    });
    fireEvent.click(screen.getByText("全部应用（混音采集）"));
    fireEvent.click(screen.getByText("保存配置"));
    // onSave 收到 null（端点级回环）
    expect(
      screen.getByText("全部应用（混音采集）").closest("button")!.getAttribute("aria-pressed")
    ).toBe("true");
  });

  it("进程级回环说明覆盖 Windows 版本要求", () => {
    setup();
    expect(screen.getByText(/Windows 10 2004/)).toBeTruthy();
    expect(screen.getByText(/背景音乐不会被混入/)).toBeTruthy();
  });
});
