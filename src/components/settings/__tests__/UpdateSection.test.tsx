import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import UpdateSection from "../UpdateSection";
import { useAppStore } from "../../../stores/appStore";

const ipcMock = vi.fn();
vi.mock("../../../lib/ipc", () => ({
  ipc: (...args: unknown[]) => ipcMock(...args),
}));

/** 依据命令名返回不同的 mock 结果 */
function mockIpc(overrides: Record<string, unknown> = {}) {
  const responses: Record<string, unknown> = {
    app_version: "0.1.0",
    get_update_config: { autoCheck: false, endpoint: "https://example.com/latest.json" },
    save_update_config: { autoCheck: false, endpoint: "https://example.com/latest.json" },
    check_app_update: {
      currentVersion: "0.1.0",
      latestVersion: "0.1.0",
      available: false,
      notes: null,
      url: null,
      message: "已是最新版本（0.1.0）",
    },
    ...overrides,
  };

  ipcMock.mockImplementation((command: string) => {
    if (command in responses) return Promise.resolve(responses[command]);
    return Promise.reject(new Error(`未 mock 的命令：${command}`));
  });
}

describe("UpdateSection", () => {
  beforeEach(() => {
    ipcMock.mockReset();
    useAppStore.setState({ locale: "zh-CN" });
    mockIpc();
  });

  it("展示当前版本与已保存的更新地址", async () => {
    render(<UpdateSection />);

    await waitFor(() => expect(screen.getByText("v0.1.0")).toBeTruthy());
    const input = screen.getByLabelText("更新地址（latest.json）") as HTMLInputElement;
    expect(input.value).toBe("https://example.com/latest.json");
  });

  it("检查更新并展示已是最新", async () => {
    render(<UpdateSection />);
    await waitFor(() => expect(screen.getByText("v0.1.0")).toBeTruthy());

    fireEvent.click(screen.getByText("检查更新"));

    await waitFor(() =>
      expect(screen.getByText("已是最新版本（0.1.0）")).toBeTruthy()
    );
    expect(ipcMock).toHaveBeenCalledWith("check_app_update", {
      endpoint: "https://example.com/latest.json",
      currentVersion: null,
    });
  });

  it("发现新版本时展示版本说明与下载地址", async () => {
    mockIpc({
      check_app_update: {
        currentVersion: "0.1.0",
        latestVersion: "0.2.0",
        available: true,
        notes: "新增迷你模式",
        url: "https://example.com/app-0.2.0.zip",
        message: "发现新版本 0.2.0（当前 0.1.0）",
      },
    });

    render(<UpdateSection />);
    await waitFor(() => expect(screen.getByText("v0.1.0")).toBeTruthy());

    fireEvent.click(screen.getByText("检查更新"));

    await waitFor(() =>
      expect(screen.getByText("发现新版本 0.2.0（当前 0.1.0）")).toBeTruthy()
    );
    expect(screen.getByText("新增迷你模式")).toBeTruthy();
    expect(screen.getByText("https://example.com/app-0.2.0.zip")).toBeTruthy();
  });

  it("检查失败时展示可读错误", async () => {
    ipcMock.mockImplementation((command: string) => {
      if (command === "app_version") return Promise.resolve("0.1.0");
      if (command === "get_update_config")
        return Promise.resolve({ autoCheck: false, endpoint: "" });
      if (command === "check_app_update")
        return Promise.reject(new Error("尚未配置更新地址（可在「设置 → 关于与更新」中填写）"));
      return Promise.reject(new Error("unexpected"));
    });

    render(<UpdateSection />);
    await waitFor(() => expect(screen.getByText("v0.1.0")).toBeTruthy());

    fireEvent.click(screen.getByText("检查更新"));

    await waitFor(() =>
      expect(screen.getByText(/尚未配置更新地址/)).toBeTruthy()
    );
  });

  it("编辑地址失焦后保存", async () => {
    render(<UpdateSection />);
    await waitFor(() => expect(screen.getByText("v0.1.0")).toBeTruthy());

    const input = screen.getByLabelText("更新地址（latest.json）");
    fireEvent.change(input, { target: { value: "https://cdn.example.com/latest.json" } });
    fireEvent.blur(input);

    await waitFor(() =>
      expect(ipcMock).toHaveBeenCalledWith("save_update_config", {
        config: {
          autoCheck: false,
          endpoint: "https://cdn.example.com/latest.json",
        },
      })
    );
  });

  it("可开启启动时自动检查", async () => {
    render(<UpdateSection />);
    await waitFor(() => expect(screen.getByText("v0.1.0")).toBeTruthy());

    fireEvent.click(screen.getByLabelText("启动时自动检查"));

    await waitFor(() =>
      expect(ipcMock).toHaveBeenCalledWith("save_update_config", {
        config: { autoCheck: true, endpoint: "https://example.com/latest.json" },
      })
    );
  });
});
