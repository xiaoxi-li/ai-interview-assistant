import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import InterfaceSettingsSection from "../InterfaceSettingsSection";
import { emptySaveFeedback, useSettingsStore } from "../../../stores/settingsStore";
import type { InterfaceConfig } from "../../../types";

const ipcMock = vi.fn();
vi.mock("../../../lib/ipc", () => ({
  ipc: (...args: unknown[]) => ipcMock(...args),
}));

const config: InterfaceConfig = {
  theme: "dark",
  hotkeys: [
    { action: "ask", accelerator: "Ctrl+Shift+A" },
    { action: "regenerate", accelerator: "Ctrl+Shift+R" },
    { action: "copy_answer", accelerator: "Ctrl+Shift+C" },
    { action: "pause_capture", accelerator: "Ctrl+Shift+P" },
    { action: "toggle_mini", accelerator: "Ctrl+Shift+M" },
  ],
  autoStart: false,
  trayBehavior: "minimize",
  alwaysOnTop: false,
  hideFromCapture: false,
};

function setup(overrides: Partial<Parameters<typeof InterfaceSettingsSection>[0]> = {}) {
  const handlers = { onSave: vi.fn(), onToggleAutoStart: vi.fn() };
  render(
    <InterfaceSettingsSection
      config={config}
      autoStartEnabled={false}
      {...handlers}
      {...overrides}
    />
  );
  return handlers;
}

describe("InterfaceSettingsSection", () => {
  it("展示五个快捷键动作与当前绑定", () => {
    setup();
    expect(screen.getByText("一键提问")).toBeTruthy();
    expect(screen.getByText("切换迷你模式")).toBeTruthy();
    expect(screen.getByText("Ctrl+Shift+A")).toBeTruthy();
    expect(screen.getByText("Ctrl+Shift+M")).toBeTruthy();
  });

  it("切换主题后保存回传", () => {
    const h = setup();
    fireEvent.click(screen.getByText("浅色"));
    fireEvent.click(screen.getByText("保存配置"));

    expect(h.onSave).toHaveBeenCalledWith(
      expect.objectContaining({ theme: "light" })
    );
  });

  it("主题三选一包含「跟随系统」（ST-13）", () => {
    const h = setup();
    expect(screen.getByText("深色")).toBeTruthy();
    expect(screen.getByText("浅色")).toBeTruthy();
    expect(screen.getByText("跟随系统")).toBeTruthy();

    fireEvent.click(screen.getByText("跟随系统"));
    fireEvent.click(screen.getByText("保存配置"));

    expect(h.onSave).toHaveBeenCalledWith(
      expect.objectContaining({ theme: "system" })
    );
  });

  it("点选主题即时预览，离开分区后回滚（ST-13）", () => {
    useSettingsStore.setState({ previewTheme: null });
    const { unmount } = render(
      <InterfaceSettingsSection
        config={config}
        autoStartEnabled={false}
        onSave={vi.fn()}
        onToggleAutoStart={vi.fn()}
      />
    );

    fireEvent.click(screen.getByText("浅色"));
    expect(useSettingsStore.getState().previewTheme).toBe("light");

    unmount();
    expect(useSettingsStore.getState().previewTheme).toBeNull();
  });

  it("录制快捷键：按下组合键后更新绑定", () => {
    const h = setup();
    const recorder = screen.getByText("Ctrl+Shift+A");
    fireEvent.click(recorder);
    expect(screen.getByText("请按下组合键…")).toBeTruthy();

    fireEvent.keyDown(recorder, { key: "k", ctrlKey: true, shiftKey: true });
    expect(screen.getByText("Ctrl+Shift+K")).toBeTruthy();

    fireEvent.click(screen.getByText("保存配置"));
    expect(h.onSave).toHaveBeenCalledWith(
      expect.objectContaining({
        hotkeys: expect.arrayContaining([
          { action: "ask", accelerator: "Ctrl+Shift+K" },
        ]),
      })
    );
  });

  it("仅有修饰键时不改变绑定", () => {
    setup();
    const recorder = screen.getByText("Ctrl+Shift+A");
    fireEvent.click(recorder);
    fireEvent.keyDown(recorder, { key: "Control", ctrlKey: true });

    expect(screen.getByText("请按下组合键…")).toBeTruthy();
  });

  it("Esc 取消录制且不改变绑定", () => {
    setup();
    const recorder = screen.getByText("Ctrl+Shift+A");
    fireEvent.click(recorder);
    fireEvent.keyDown(recorder, { key: "Escape" });

    expect(screen.getByText("Ctrl+Shift+A")).toBeTruthy();
  });

  it("快捷键冲突时提示并禁用保存", () => {
    setup();
    const recorder = screen.getByText("Ctrl+Shift+R");
    fireEvent.click(recorder);
    fireEvent.keyDown(recorder, { key: "a", ctrlKey: true, shiftKey: true });

    expect(screen.getByText(/快捷键冲突/)).toBeTruthy();
    const save = screen.getByText("保存配置") as HTMLButtonElement;
    expect(save.disabled).toBe(true);
  });

  it("Backspace 清空绑定并提示必填", () => {
    setup();
    const recorder = screen.getByText("Ctrl+Shift+A");
    fireEvent.click(recorder);
    fireEvent.keyDown(recorder, { key: "Backspace" });

    expect(screen.getByText("未设置")).toBeTruthy();
    fireEvent.click(screen.getByText("保存配置"));
    expect(screen.getByText(/不能为空/)).toBeTruthy();
  });

  it("切换关闭窗口行为后保存", () => {
    const h = setup();
    fireEvent.change(screen.getByLabelText(/关闭窗口行为/), {
      target: { value: "quit" },
    });
    fireEvent.click(screen.getByText("保存配置"));

    expect(h.onSave).toHaveBeenCalledWith(
      expect.objectContaining({ trayBehavior: "quit" })
    );
  });

  it("开机自启开关触发回调", () => {
    const h = setup();
    fireEvent.click(screen.getByLabelText("开机自启"));
    expect(h.onToggleAutoStart).toHaveBeenCalledWith(true);
  });

  it("外部自启状态变化反映到开关", () => {
    setup({ autoStartEnabled: true });
    const checkbox = screen.getByLabelText("开机自启") as HTMLInputElement;
    expect(checkbox.checked).toBe(true);
  });

  it("开机自启开关在该分区就地提示结果（ST-11）", async () => {
    ipcMock.mockReset();
    useSettingsStore.setState({ saveFeedback: emptySaveFeedback(), error: null });
    ipcMock.mockResolvedValueOnce(true);

    render(
      <InterfaceSettingsSection
        config={config}
        autoStartEnabled={false}
        onSave={vi.fn()}
        onToggleAutoStart={useSettingsStore.getState().setAutoStart}
      />
    );

    fireEvent.click(screen.getByLabelText("开机自启"));

    expect(await screen.findByText("开机自启已开启")).toBeTruthy();
    expect(useSettingsStore.getState().saveFeedback.interface?.kind).toBe("success");
  });
});
