import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../ipc", () => ({ ipc: vi.fn() }));

import { ipc } from "../ipc";
import {
  emptySaveFeedback,
  useSettingsStore,
  type SettingsFeedbackTarget,
} from "../../stores/settingsStore";
import type {
  AIConfig,
  ASRConfig,
  AudioConfig,
  InterfaceConfig,
  KnowledgeConfig,
  PrivacyConfig,
} from "../../types";

const mockedIpc = vi.mocked(ipc);

describe("settingsStore 密钥管理", () => {
  beforeEach(() => {
    mockedIpc.mockReset();
    useSettingsStore.setState({
      apiKeys: {},
      error: null,
      testing: false,
      testResult: null,
      testingAsr: false,
      asrTestResult: null,
    });
  });

  it("云端 ASR 连接测试成功后记录结果（IA-16.8）", async () => {
    mockedIpc.mockResolvedValueOnce({
      ok: true,
      latencyMs: 128,
      message: "阿里云智能语音 连接成功（128ms）",
    });
    const draft = {
      mode: "cloud" as const,
      cloudProvider: "aliyun" as const,
      model: "qwen-audio-3.1-asr-flash-streaming",
      language: "zh-CN",
      vadThreshold: 0.5,
      localModel: "sherpa-onnx-paraformer-zh",
      cloudBaseUrls: {},
    };

    await useSettingsStore.getState().testAsrConnection(draft);

    // 传草稿时测的就是草稿（而非已保存配置）
    expect(mockedIpc).toHaveBeenCalledWith("test_cloud_asr_connection", {
      config: draft,
    });
    expect(useSettingsStore.getState().testingAsr).toBe(false);
    expect(useSettingsStore.getState().asrTestResult?.ok).toBe(true);
    expect(useSettingsStore.getState().asrTestResult?.latencyMs).toBe(128);
  });

  it("云端 ASR 连接测试异常时记录失败结果", async () => {
    mockedIpc.mockRejectedValueOnce(new Error("connection refused"));
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});

    await useSettingsStore.getState().testAsrConnection();

    const result = useSettingsStore.getState().asrTestResult;
    expect(result?.ok).toBe(false);
    expect(result?.message).toContain("connection refused");
    expect(useSettingsStore.getState().testingAsr).toBe(false);
    spy.mockRestore();
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
      platform: "openai",
      configured: false,
      masked: "",
    });
    await useSettingsStore.getState().loadApiKeyStatus("openai");

    const keys = useSettingsStore.getState().apiKeys;
    expect(keys.deepseek.configured).toBe(true);
    expect(keys.openai.configured).toBe(false);
  });
});

describe("settingsStore 设置保存反馈（ST-11）", () => {
  const aiConfig: AIConfig = {
    platform: "deepseek",
    baseUrl: "https://api.deepseek.com/v1",
    model: "deepseek-chat",
    stream: true,
    temperature: 0.7,
  };
  const asrConfig: ASRConfig = {
    mode: "local",
    cloudProvider: "aliyun",
    model: "qwen-audio-3.1-asr-flash-streaming",
    language: "zh-CN",
    vadThreshold: 0.5,
    localModel: "sherpa-onnx-paraformer-zh",
  };
  const audioConfig: AudioConfig = { micDeviceId: null, systemDeviceId: null };
  const knowledgeConfig: KnowledgeConfig = {
    dbPath: null,
    modelsDir: null,
    embeddingModel: "bge-m3",
  };
  const interfaceConfig: InterfaceConfig = {
    theme: "dark",
    hotkeys: [{ action: "ask", accelerator: "Ctrl+Shift+A" }],
    autoStart: false,
    trayBehavior: "minimize",
    alwaysOnTop: false,
    hideFromCapture: false,
  };
  const privacyConfig: PrivacyConfig = { uploadAudio: false, encryptLocal: false };
  const credentialStatus = {
    provider: "aliyun",
    complete: true,
    fields: [{ field: "accessKeyId", configured: true, masked: "ak****" }],
  };

  const feedbackOf = (target: SettingsFeedbackTarget) =>
    useSettingsStore.getState().saveFeedback[target];

  beforeEach(() => {
    mockedIpc.mockReset();
    useSettingsStore.setState({ error: null, saveFeedback: emptySaveFeedback() });
  });

  it("初始状态下所有分区均无反馈", () => {
    expect(useSettingsStore.getState().saveFeedback).toEqual(emptySaveFeedback());
  });

  it("AI 分区：保存配置 / 保存密钥 / 清除密钥 均写入成功反馈", async () => {
    mockedIpc.mockResolvedValueOnce(aiConfig);
    await useSettingsStore.getState().saveAiConfig(aiConfig);
    expect(feedbackOf("ai")).toEqual({ kind: "success", text: "AI 配置已保存" });

    mockedIpc.mockResolvedValueOnce({
      platform: "deepseek",
      configured: true,
      masked: "sk-****",
    });
    await useSettingsStore.getState().saveApiKey("deepseek", "sk-abcdef");
    expect(feedbackOf("ai")).toEqual({ kind: "success", text: "密钥已保存" });

    mockedIpc.mockResolvedValueOnce({
      platform: "deepseek",
      configured: false,
      masked: "",
    });
    await useSettingsStore.getState().deleteApiKey("deepseek");
    expect(feedbackOf("ai")).toEqual({ kind: "success", text: "密钥已清除" });
  });

  it("ASR 分区：保存配置 / 保存凭证 / 清空凭证", async () => {
    mockedIpc.mockResolvedValueOnce(asrConfig);
    await useSettingsStore.getState().saveAsrConfig(asrConfig);
    expect(feedbackOf("asr")).toEqual({ kind: "success", text: "ASR 配置已保存" });

    mockedIpc.mockResolvedValueOnce(credentialStatus);
    await useSettingsStore.getState().saveAsrCredential("aliyun", "accessKeyId", "ak-1");
    expect(feedbackOf("asr")).toEqual({ kind: "success", text: "凭证已保存" });

    mockedIpc.mockResolvedValueOnce({ provider: "aliyun", complete: false, fields: [] });
    await useSettingsStore.getState().deleteAsrCredentials("aliyun");
    expect(feedbackOf("asr")).toEqual({ kind: "success", text: "凭证已清空" });
  });

  it("音频分区：保存配置 / 刷新设备", async () => {
    mockedIpc.mockResolvedValueOnce(audioConfig);
    await useSettingsStore.getState().saveAudioConfig(audioConfig);
    expect(feedbackOf("audio")).toEqual({ kind: "success", text: "音频配置已保存" });

    mockedIpc.mockResolvedValueOnce([]);
    await useSettingsStore.getState().refreshAudioDevices();
    expect(feedbackOf("audio")).toEqual({ kind: "success", text: "设备列表已刷新" });
  });

  it("知识库与隐私分区：各自写入反馈且互不串台", async () => {
    mockedIpc.mockResolvedValueOnce(knowledgeConfig);
    await useSettingsStore.getState().saveKnowledgeConfig(knowledgeConfig);
    expect(feedbackOf("knowledge")).toEqual({
      kind: "success",
      text: "知识库配置已保存",
    });
    expect(feedbackOf("privacy")).toBeNull();

    mockedIpc.mockResolvedValueOnce(privacyConfig);
    await useSettingsStore.getState().savePrivacyConfig(privacyConfig);
    expect(feedbackOf("privacy")).toEqual({ kind: "success", text: "隐私配置已保存" });
  });

  it("界面分区：保存配置与开机自启开关（区分开启 / 关闭文案）", async () => {
    mockedIpc.mockResolvedValueOnce(interfaceConfig);
    await useSettingsStore.getState().saveInterfaceConfig(interfaceConfig);
    expect(feedbackOf("interface")).toEqual({ kind: "success", text: "界面配置已保存" });

    mockedIpc.mockResolvedValueOnce(true);
    await useSettingsStore.getState().setAutoStart(true);
    expect(feedbackOf("interface")).toEqual({ kind: "success", text: "开机自启已开启" });

    mockedIpc.mockResolvedValueOnce(false);
    await useSettingsStore.getState().setAutoStart(false);
    expect(feedbackOf("interface")).toEqual({ kind: "success", text: "开机自启已关闭" });
  });

  it("失败时写入失败反馈，且既有 error 字段语义不变", async () => {
    mockedIpc.mockRejectedValueOnce(new Error("keychain unavailable"));
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});

    await useSettingsStore.getState().saveApiKey("qwen", "sk-test");

    const feedback = feedbackOf("ai");
    expect(feedback?.kind).toBe("error");
    expect(feedback?.text).toContain("保存密钥失败");
    expect(feedback?.text).toContain("keychain unavailable");
    // 既有语义：error 仍被写入（settingsStore.test 既有断言依赖）
    expect(useSettingsStore.getState().error).toContain("keychain unavailable");
    // 其他分区不受影响
    expect(feedbackOf("asr")).toBeNull();
    spy.mockRestore();
  });

  it("clearSaveFeedback 只清空指定分区", async () => {
    mockedIpc.mockResolvedValueOnce(aiConfig);
    await useSettingsStore.getState().saveAiConfig(aiConfig);
    mockedIpc.mockResolvedValueOnce(audioConfig);
    await useSettingsStore.getState().saveAudioConfig(audioConfig);

    useSettingsStore.getState().clearSaveFeedback("ai");

    expect(feedbackOf("ai")).toBeNull();
    expect(feedbackOf("audio")?.kind).toBe("success");
  });

  it("setSaveFeedback 可手动写入指定分区", () => {
    useSettingsStore
      .getState()
      .setSaveFeedback("knowledge", { kind: "error", text: "路径不可写" });

    expect(feedbackOf("knowledge")).toEqual({ kind: "error", text: "路径不可写" });
  });
});
