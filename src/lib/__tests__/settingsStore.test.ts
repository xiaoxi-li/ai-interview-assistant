import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../ipc", () => ({ ipc: vi.fn() }));

import { ipc } from "../ipc";
import { useSettingsStore } from "../../stores/settingsStore";

const mockedIpc = vi.mocked(ipc);

describe("settingsStore 密钥管理", () => {
  beforeEach(() => {
    mockedIpc.mockReset();
    useSettingsStore.setState({
      apiKeys: {},
      error: null,
      testing: false,
      testResult: null,
    });
  });

  it("连接测试成功后记录结果并复位 loading", async () => {
    mockedIpc.mockResolvedValueOnce({
      ok: true,
      latencyMs: 42,
      message: "连接成功（HTTP 200）",
    });

    await useSettingsStore.getState().testAiConnection();

    expect(mockedIpc).toHaveBeenCalledWith("test_ai_connection");
    expect(useSettingsStore.getState().testing).toBe(false);
    expect(useSettingsStore.getState().testResult?.ok).toBe(true);
    expect(useSettingsStore.getState().testResult?.latencyMs).toBe(42);
  });

  it("连接测试异常时记录失败结果", async () => {
    mockedIpc.mockRejectedValueOnce(new Error("network down"));
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});

    await useSettingsStore.getState().testAiConnection();

    const result = useSettingsStore.getState().testResult;
    expect(result?.ok).toBe(false);
    expect(result?.message).toContain("network down");
    expect(useSettingsStore.getState().testing).toBe(false);
    spy.mockRestore();
  });

  it("查询密钥状态并写入 state", async () => {
    mockedIpc.mockResolvedValueOnce({
      platform: "deepseek",
      configured: true,
      masked: "sk-****cdef",
    });

    await useSettingsStore.getState().loadApiKeyStatus("deepseek");

    expect(mockedIpc).toHaveBeenCalledWith("get_api_key_status", {
      platform: "deepseek",
    });
    expect(useSettingsStore.getState().apiKeys.deepseek.configured).toBe(true);
    expect(useSettingsStore.getState().apiKeys.deepseek.masked).toBe("sk-****cdef");
  });

  it("保存密钥后更新状态（仅存掩码）", async () => {
    mockedIpc.mockResolvedValueOnce({
      platform: "glm",
      configured: true,
      masked: "abc****wxyz",
    });

    await useSettingsStore.getState().saveApiKey("glm", "abc123456wxyz");

    expect(mockedIpc).toHaveBeenCalledWith("save_api_key", {
      platform: "glm",
      apiKey: "abc123456wxyz",
    });
    const status = useSettingsStore.getState().apiKeys.glm;
    expect(status.configured).toBe(true);
    // 前端不保存完整密钥
    expect(JSON.stringify(status)).not.toContain("123456");
  });

  it("删除密钥后状态变为未配置", async () => {
    mockedIpc.mockResolvedValueOnce({
      platform: "deepseek",
      configured: false,
      masked: "",
    });

    await useSettingsStore.getState().deleteApiKey("deepseek");

    expect(mockedIpc).toHaveBeenCalledWith("delete_api_key", {
      platform: "deepseek",
    });
    expect(useSettingsStore.getState().apiKeys.deepseek.configured).toBe(false);
  });

  it("失败时记录错误", async () => {
    mockedIpc.mockRejectedValueOnce(new Error("keychain unavailable"));
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});

    await useSettingsStore.getState().saveApiKey("qwen", "sk-test");

    expect(useSettingsStore.getState().error).toContain("keychain unavailable");
    spy.mockRestore();
  });

  it("多平台状态互不覆盖", async () => {
    mockedIpc.mockResolvedValueOnce({
      platform: "deepseek",
      configured: true,
      masked: "a****b",
    });
    await useSettingsStore.getState().loadApiKeyStatus("deepseek");

    mockedIpc.mockResolvedValueOnce({
      platform: "chatgpt",
      configured: false,
      masked: "",
    });
    await useSettingsStore.getState().loadApiKeyStatus("chatgpt");

    const keys = useSettingsStore.getState().apiKeys;
    expect(keys.deepseek.configured).toBe(true);
    expect(keys.chatgpt.configured).toBe(false);
  });
});
