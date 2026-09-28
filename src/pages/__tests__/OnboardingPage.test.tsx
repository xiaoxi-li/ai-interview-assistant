import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import OnboardingPage from "../OnboardingPage";
import { useAppStore } from "../../stores/appStore";
import { useSettingsStore } from "../../stores/settingsStore";
import type { AIConfig, ModelStatus, PlatformPreset } from "../../types";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

const aiConfig: AIConfig = {
  platform: "deepseek",
  baseUrl: "https://api.deepseek.com/v1",
  model: "deepseek-chat",
  stream: true,
  temperature: 0.7,
  thinking: "nonThinking",
};

const presets: PlatformPreset[] = [
  {
    platform: "deepseek",
    label: "DeepSeek",
    defaultBaseUrl: "https://api.deepseek.com/v1",
    defaultModel: "deepseek-chat",
    thinkingParam: null,
  },
];

const asrModel: ModelStatus = {
  id: "asr-paraformer-zh",
  name: "本地语音识别（Paraformer 中文）",
  kind: "asr",
  repo: "pengzhendong/sherpa-onnx-paraformer-zh",
  files: [
    {
      remotePath: "model.int8.onnx",
      localName: "model.int8.onnx",
      sha256: "0".repeat(64),
      sizeBytes: 200 * 1024 * 1024,
    },
    {
      remotePath: "tokens.txt",
      localName: "tokens.txt",
      sha256: "",
      sizeBytes: 75_354,
    },
  ],
  description: "中文流式识别，离线可用",
  requiredForLocalAsr: true,
  installed: false,
  resolvedPath: null,
  partialBytes: 0,
};

function resetStores(overrides: {
  model?: Partial<ModelStatus>;
  progress?: boolean;
} = {}) {
  useAppStore.setState({
    completeOnboarding: vi.fn().mockResolvedValue(undefined),
  } as Partial<ReturnType<typeof useAppStore.getState>>);

  useSettingsStore.setState({
    aiConfig,
    presets,
    apiKeys: {
      deepseek: { platform: "deepseek", configured: true, masked: "sk-***abc" },
    },
    testing: false,
    testResult: null,
    models: [{ ...asrModel, ...(overrides.model ?? {}) }],
    modelProgress: overrides.progress
      ? {
          "asr-paraformer-zh": {
            id: "asr-paraformer-zh",
            downloaded: 50 * 1024 * 1024,
            total: 100 * 1024 * 1024,
          },
        }
      : {},
    downloadingId: null,
    loadAiSettings: vi.fn().mockResolvedValue(undefined),
    saveAiConfig: vi.fn().mockResolvedValue(undefined),
    saveApiKey: vi.fn().mockResolvedValue(undefined),
    deleteApiKey: vi.fn().mockResolvedValue(undefined),
    testAiConnection: vi.fn().mockResolvedValue(undefined),
    loadApiKeyStatus: vi.fn().mockResolvedValue(undefined),
    loadModels: vi.fn().mockResolvedValue(undefined),
    downloadModel: vi.fn().mockResolvedValue(undefined),
    applyModelProgress: vi.fn(),
  } as unknown as Partial<ReturnType<typeof useSettingsStore.getState>>);
}

describe("OnboardingPage", () => {
  beforeEach(() => {
    resetStores();
  });

  it("默认展示第一步的隐私说明", () => {
    render(<OnboardingPage />);

    expect(screen.getByText("欢迎使用轻松面试")).toBeTruthy();
    expect(screen.getByText("第 1 / 4 步 · 本地优先与权限")).toBeTruthy();
    expect(screen.getByText(/不会上传/)).toBeTruthy();
  });

  it("第一步禁用「上一步」", () => {
    render(<OnboardingPage />);
    const button = screen.getByText("上一步").closest("button") as HTMLButtonElement;
    expect(button.disabled).toBe(true);
  });

  it("可逐步前进与后退", () => {
    render(<OnboardingPage />);

    fireEvent.click(screen.getByText("下一步"));
    expect(screen.getByText("第 2 / 4 步 · 语音识别通路")).toBeTruthy();
    expect(screen.getByText(/默认使用本地语音识别/)).toBeTruthy();

    fireEvent.click(screen.getByText("下一步"));
    expect(screen.getByText("第 3 / 4 步 · AI 平台配置")).toBeTruthy();
    expect(screen.getByText(/配置一个大模型服务/)).toBeTruthy();

    fireEvent.click(screen.getByText("上一步"));
    expect(screen.getByText("第 2 / 4 步 · 语音识别通路")).toBeTruthy();
  });

  it("ASR 步骤展示模型状态并支持下载", () => {
    const downloadModel = vi.fn().mockResolvedValue(undefined);
    useSettingsStore.setState({
      downloadModel,
    } as unknown as Partial<ReturnType<typeof useSettingsStore.getState>>);

    render(<OnboardingPage />);
    fireEvent.click(screen.getByText("下一步"));

    expect(screen.getByText("本地语音识别（Paraformer 中文）")).toBeTruthy();
    expect(screen.getByText("待下载")).toBeTruthy();

    fireEvent.click(screen.getByText("立即下载模型"));
    expect(downloadModel).toHaveBeenCalledWith("asr-paraformer-zh");
  });

  it("模型已就绪时不显示下载按钮", () => {
    resetStores({ model: { installed: true } });
    render(<OnboardingPage />);
    fireEvent.click(screen.getByText("下一步"));

    expect(screen.getByText("已就绪")).toBeTruthy();
    expect(screen.queryByText("立即下载模型")).toBeNull();
  });

  it("下载中展示进度", () => {
    resetStores({ progress: true });
    useSettingsStore.setState({
      downloadingId: "asr-paraformer-zh",
    } as unknown as Partial<ReturnType<typeof useSettingsStore.getState>>);

    render(<OnboardingPage />);
    fireEvent.click(screen.getByText("下一步"));

    expect(screen.getByText(/50\.0 MB \/ 100\.0 MB（50%）/)).toBeTruthy();
    expect(screen.getByText(/正在下载，可稍后在/)).toBeTruthy();
  });

  it("AI 步骤内嵌平台配置表单", () => {
    render(<OnboardingPage />);
    fireEvent.click(screen.getByText("下一步"));
    fireEvent.click(screen.getByText("下一步"));

    expect(screen.getByDisplayValue("https://api.deepseek.com/v1")).toBeTruthy();
    expect(screen.getByDisplayValue("deepseek-chat")).toBeTruthy();
  });

  it("最后一步总结并完成向导", () => {
    const completeOnboarding = vi.fn().mockResolvedValue(undefined);
    useAppStore.setState({
      completeOnboarding,
    } as Partial<ReturnType<typeof useAppStore.getState>>);

    render(<OnboardingPage />);
    for (let index = 0; index < 3; index += 1) {
      fireEvent.click(screen.getByText("下一步"));
    }

    expect(screen.getByText("第 4 / 4 步 · 完成")).toBeTruthy();
    expect(screen.getByText(/尚未下载模型/)).toBeTruthy();
    expect(screen.getByText(/deepseek \/ deepseek-chat/)).toBeTruthy();

    fireEvent.click(screen.getByText("开始使用"));
    expect(completeOnboarding).toHaveBeenCalledTimes(1);
  });

  it("可跳过向导", () => {
    const completeOnboarding = vi.fn().mockResolvedValue(undefined);
    useAppStore.setState({
      completeOnboarding,
    } as Partial<ReturnType<typeof useAppStore.getState>>);

    render(<OnboardingPage />);
    fireEvent.click(screen.getByText("跳过向导"));
    expect(completeOnboarding).toHaveBeenCalledTimes(1);
  });
});
