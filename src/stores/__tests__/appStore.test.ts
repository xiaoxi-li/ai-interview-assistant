import { beforeEach, describe, expect, it, vi } from "vitest";

const ipcMock = vi.fn();
vi.mock("../../lib/ipc", () => ({
  ipc: (...args: unknown[]) => ipcMock(...args),
}));

import { useAppStore } from "../appStore";

function enableTauri(value = true) {
  const target = window as unknown as Record<string, unknown>;
  if (value) target.__TAURI_INTERNALS__ = {};
  else delete target.__TAURI_INTERNALS__;
}

describe("appStore 迷你模式（IA-13）", () => {
  beforeEach(() => {
    ipcMock.mockReset();
    ipcMock.mockResolvedValue(null);
    vi.spyOn(console, "error").mockImplementation(() => {});
    enableTauri();
    useAppStore.setState({ miniMode: false, currentPage: "knowledge" });
  });

  it("进入迷你模式切换视图并调用后端命令", () => {
    useAppStore.getState().enterMiniMode();

    expect(useAppStore.getState().miniMode).toBe(true);
    expect(ipcMock).toHaveBeenCalledWith("enter_mini_mode");
  });

  it("重复进入为幂等，不重复调用后端", () => {
    useAppStore.getState().enterMiniMode();
    useAppStore.getState().enterMiniMode();

    expect(ipcMock).toHaveBeenCalledTimes(1);
  });

  it("退出迷你模式回到面试辅助页并调用后端", () => {
    useAppStore.setState({ miniMode: true, currentPage: "settings" });
    useAppStore.getState().exitMiniMode();

    expect(useAppStore.getState().miniMode).toBe(false);
    expect(useAppStore.getState().currentPage).toBe("interview");
    expect(ipcMock).toHaveBeenCalledWith("exit_mini_mode");
  });

  it("未处于迷你模式时退出为空操作", () => {
    useAppStore.getState().exitMiniMode();

    expect(useAppStore.getState().miniMode).toBe(false);
    expect(ipcMock).not.toHaveBeenCalled();
  });

  it("切换迷你模式可在进入与退出之间往返", () => {
    useAppStore.getState().toggleMiniMode();
    expect(useAppStore.getState().miniMode).toBe(true);

    useAppStore.getState().toggleMiniMode();
    expect(useAppStore.getState().miniMode).toBe(false);
  });

  it("后端进入命令失败时回退完整界面", async () => {
    ipcMock.mockRejectedValueOnce(new Error("窗口不可用"));
    useAppStore.getState().enterMiniMode();

    expect(useAppStore.getState().miniMode).toBe(true);
    await vi.waitFor(() => expect(useAppStore.getState().miniMode).toBe(false));
  });

  it("非 Tauri 环境仅切换视图，不调用后端命令", () => {
    enableTauri(false);
    useAppStore.getState().enterMiniMode();

    expect(useAppStore.getState().miniMode).toBe(true);
    expect(ipcMock).not.toHaveBeenCalled();
  });
});
