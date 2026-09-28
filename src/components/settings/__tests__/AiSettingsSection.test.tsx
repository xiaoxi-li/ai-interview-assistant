import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import AiSettingsSection from "../AiSettingsSection";
import { emptySaveFeedback, useSettingsStore } from "../../../stores/settingsStore";
import type { AIConfig, PlatformPreset } from "../../../types";

const ipcMock = vi.fn();
vi.mock("../../../lib/ipc", () => ({
  ipc: (...args: unknown[]) => ipcMock(...args),
}));

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

  it("配置非法时展示校验错误并禁用保存", () => {
    setup();
    fireEvent.change(screen.getByLabelText("模型名"), { target: { value: "  " } });

    expect(screen.getByText("模型名不能为空")).toBeTruthy();
    const saveButton = screen.getByText("保存配置") as HTMLButtonElement;
    expect(saveButton.disabled).toBe(true);
  });

  it("修改后未保存时提示未保存状态", () => {
    setup();
    fireEvent.change(screen.getByLabelText("模型名"), {
      target: { value: "deepseek-reasoner" },
    });
    expect(screen.getByText("有未保存的修改")).toBeTruthy();
  });

  it("点击测试连接触发回调", () => {
    const onTestConnection = vi.fn();
    setup({ onTestConnection });

    fireEvent.click(screen.getByText("测试连接"));

    expect(onTestConnection).toHaveBeenCalledTimes(1);
  });

  it("测试中禁用按钮并展示进行中文案", () => {
    const onTestConnection = vi.fn();
    setup({ onTestConnection, testing: true });

    const button = screen.getByText("测试中…") as HTMLButtonElement;
    expect(button.disabled).toBe(true);
    fireEvent.click(button);
    expect(onTestConnection).not.toHaveBeenCalled();
  });

  it("展示成功结果与耗时", () => {
    setup({
      onTestConnection: vi.fn(),
      testResult: { ok: true, latencyMs: 42, message: "连接成功（HTTP 200）" },
    });

    expect(screen.getByText("连接成功（HTTP 200）")).toBeTruthy();
    expect(screen.getByText("42 ms")).toBeTruthy();
  });

  it("展示失败结果且不显示耗时", () => {
    setup({
      onTestConnection: vi.fn(),
      testResult: { ok: false, latencyMs: 0, message: "服务返回错误：invalid api key" },
    });

    expect(screen.getByText("服务返回错误：invalid api key")).toBeTruthy();
    expect(screen.queryByText("0 ms")).toBeNull();
  });

  it("已配置时占位符显示掩码", () => {
    setup({
      apiKeyStatus: { platform: "deepseek", configured: true, masked: "sk-****cdef" },
    });
    expect(
      screen.getByPlaceholderText("已配置：sk-****cdef")
    ).toBeTruthy();
  });

  it("保存配置成功 / 失败时在分区内就地提示，失败可手动关闭（ST-11）", async () => {
    ipcMock.mockReset();
    useSettingsStore.setState({ saveFeedback: emptySaveFeedback(), error: null });
    ipcMock.mockResolvedValueOnce(config);

    render(
      <AiSettingsSection
        config={config}
        presets={presets}
        onSaveConfig={useSettingsStore.getState().saveAiConfig}
        onSaveApiKey={vi.fn()}
        onDeleteApiKey={vi.fn()}
      />
    );

    fireEvent.click(screen.getByText("保存配置"));
    expect(await screen.findByText("AI 配置已保存")).toBeTruthy();

    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    ipcMock.mockRejectedValueOnce(new Error("network down"));
    fireEvent.click(screen.getByText("保存配置"));
    expect(await screen.findByText("保存 AI 配置失败：network down")).toBeTruthy();

    fireEvent.click(screen.getByLabelText("关闭提示"));
    expect(screen.queryByText("保存 AI 配置失败：network down")).toBeNull();
    spy.mockRestore();
  });

  it("保存 API Key 后提示「密钥已保存」（ST-11）", async () => {
    ipcMock.mockReset();
    useSettingsStore.setState({ saveFeedback: emptySaveFeedback(), error: null });
    ipcMock.mockResolvedValueOnce({
      platform: "deepseek",
      configured: true,
      masked: "sk-****",
    });

    render(
      <AiSettingsSection
        config={config}
        presets={presets}
        onSaveConfig={vi.fn()}
        onSaveApiKey={useSettingsStore.getState().saveApiKey}
        onDeleteApiKey={vi.fn()}
      />
    );

    fireEvent.change(screen.getByLabelText(/API Key/), {
      target: { value: "sk-test-key" },
    });
    fireEvent.click(screen.getByText("保存"));

    expect(await screen.findByText("密钥已保存")).toBeTruthy();
  });
});
