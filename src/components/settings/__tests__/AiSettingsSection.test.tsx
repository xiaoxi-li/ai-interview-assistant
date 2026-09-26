import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import AiSettingsSection from "../AiSettingsSection";
import type { AIConfig, PlatformPreset } from "../../../types";

const presets: PlatformPreset[] = [
  {
    platform: "deepseek",
    label: "DeepSeek",
    defaultBaseUrl: "https://api.deepseek.com/v1",
    defaultModel: "deepseek-chat",
  },
  {
    platform: "glm",
    label: "智谱 GLM",
    defaultBaseUrl: "https://open.bigmodel.cn/api/paas/v4",
    defaultModel: "glm-4-plus",
  },
];

const config: AIConfig = {
  platform: "deepseek",
  baseUrl: "https://api.deepseek.com/v1",
  model: "deepseek-chat",
  stream: true,
  temperature: 0.7,
};

function setup(overrides: Partial<Parameters<typeof AiSettingsSection>[0]> = {}) {
  const handlers = {
    onSaveConfig: vi.fn(),
    onSaveApiKey: vi.fn(),
    onDeleteApiKey: vi.fn(),
  };
  render(
    <AiSettingsSection
      config={config}
      presets={presets}
      {...handlers}
      {...overrides}
    />
  );
  return handlers;
}

describe("AiSettingsSection", () => {
  it("展示平台按钮与当前配置", () => {
    setup();
    expect(screen.getByText("DeepSeek")).toBeTruthy();
    expect(screen.getByText("智谱 GLM")).toBeTruthy();
    expect((screen.getByLabelText("Base URL") as HTMLInputElement).value).toBe(
      "https://api.deepseek.com/v1"
    );
    expect((screen.getByLabelText("模型名") as HTMLInputElement).value).toBe(
      "deepseek-chat"
    );
  });

  it("切换平台时套用预设 Base URL 与模型", () => {
    setup();
    fireEvent.click(screen.getByText("智谱 GLM"));

    expect((screen.getByLabelText("Base URL") as HTMLInputElement).value).toBe(
      "https://open.bigmodel.cn/api/paas/v4"
    );
    expect((screen.getByLabelText("模型名") as HTMLInputElement).value).toBe(
      "glm-4-plus"
    );
  });

  it("保存配置回传修改后的草稿", () => {
    const h = setup();
    fireEvent.change(screen.getByLabelText("模型名"), {
      target: { value: "deepseek-reasoner" },
    });
    fireEvent.click(screen.getByText("保存配置"));

    expect(h.onSaveConfig).toHaveBeenCalledWith(
      expect.objectContaining({ model: "deepseek-reasoner", platform: "deepseek" })
    );
  });

  it("温度滑块可调整", () => {
    const h = setup();
    fireEvent.change(screen.getByLabelText(/温度/), { target: { value: "1.5" } });
    fireEvent.click(screen.getByText("保存配置"));

    expect(h.onSaveConfig).toHaveBeenCalledWith(
      expect.objectContaining({ temperature: 1.5 })
    );
  });

  it("保存 API Key 后清空输入", () => {
    const h = setup();
    const input = screen.getByLabelText(/API Key/) as HTMLInputElement;

    fireEvent.change(input, { target: { value: "sk-test-key" } });
    fireEvent.click(screen.getByText("保存"));

    expect(h.onSaveApiKey).toHaveBeenCalledWith("deepseek", "sk-test-key");
    expect(input.value).toBe("");
  });

  it("空密钥不触发保存", () => {
    const h = setup();
    const saveButton = screen.getByText("保存") as HTMLButtonElement;
    expect(saveButton.disabled).toBe(true);
    fireEvent.click(saveButton);
    expect(h.onSaveApiKey).not.toHaveBeenCalled();
  });

  it("清除密钥按当前平台触发", () => {
    const h = setup();
    fireEvent.click(screen.getByText("清除"));
    expect(h.onDeleteApiKey).toHaveBeenCalledWith("deepseek");
  });

  it("已配置时占位符显示掩码", () => {
    setup({
      apiKeyStatus: { platform: "deepseek", configured: true, masked: "sk-****cdef" },
    });
    expect(
      screen.getByPlaceholderText("已配置：sk-****cdef")
    ).toBeTruthy();
  });
});
