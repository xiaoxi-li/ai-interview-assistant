import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import AsrSettingsSection from "../AsrSettingsSection";
import type { ASRConfig, AsrCredentialStatus, AsrProviderPreset } from "../../../types";

const providers: AsrProviderPreset[] = [
  {
    provider: "xfyun",
    label: "讯飞开放平台",
    defaultModel: "iat",
    credentialFields: ["app_id", "api_key", "api_secret"],
  },
  {
    provider: "tencent",
    label: "腾讯云语音识别",
    defaultModel: "16k_zh",
    credentialFields: ["secret_id", "secret_key"],
  },
];

const config: ASRConfig = {
  mode: "cloud",
  cloudProvider: "xfyun",
  model: "iat",
  language: "zh-CN",
  vadThreshold: 0.5,
  localModel: "sherpa-onnx-paraformer-zh",
};

const credentialStatus: AsrCredentialStatus = {
  provider: "xfyun",
  complete: false,
  fields: [
    { field: "app_id", configured: true, masked: "ab****yz" },
    { field: "api_key", configured: false, masked: "" },
    { field: "api_secret", configured: false, masked: "" },
  ],
};

function setup(overrides: Partial<Parameters<typeof AsrSettingsSection>[0]> = {}) {
  const handlers = {
    onSaveConfig: vi.fn(),
    onSaveCredential: vi.fn(),
    onDeleteCredentials: vi.fn(),
    onProviderChange: vi.fn(),
  };
  render(
    <AsrSettingsSection
      config={config}
      providers={providers}
      credentialStatus={credentialStatus}
      {...handlers}
      {...overrides}
    />
  );
  return handlers;
}

describe("AsrSettingsSection", () => {
  it("云端模式展示服务商与凭证字段", () => {
    setup();
    expect(screen.getByText("讯飞开放平台")).toBeTruthy();
    expect(screen.getByLabelText("AppID")).toBeTruthy();
    expect(screen.getByLabelText("API Key")).toBeTruthy();
    expect(screen.getByLabelText("API Secret")).toBeTruthy();
  });

  it("已配置字段展示掩码，缺失字段给出提示", () => {
    setup();
    expect(screen.getByPlaceholderText("已配置：ab****yz")).toBeTruthy();
    expect(screen.getByText(/缺少 API Key、API Secret/)).toBeTruthy();
  });

  it("切换到本地模式后隐藏凭证并展示本地模型", () => {
    setup();
    fireEvent.click(screen.getByText("本地（隐私优先）"));

    expect(screen.queryByLabelText("AppID")).toBeNull();
    expect(screen.getByLabelText("本地模型")).toBeTruthy();
  });

  it("保存配置回传草稿（含模式与阈值）", () => {
    const h = setup();
    fireEvent.click(screen.getByText("本地（隐私优先）"));
    fireEvent.change(screen.getByLabelText(/VAD 阈值/), { target: { value: "0.8" } });
    fireEvent.click(screen.getByText("保存配置"));

    expect(h.onSaveConfig).toHaveBeenCalledWith(
      expect.objectContaining({ mode: "local", vadThreshold: 0.8 })
    );
  });

  it("保存凭证后清空输入", () => {
    const h = setup();
    const input = screen.getByLabelText("API Key") as HTMLInputElement;

    fireEvent.change(input, { target: { value: "secret-value" } });
    const saveButtons = screen.getAllByText("保存");
    fireEvent.click(saveButtons[1]);

    expect(h.onSaveCredential).toHaveBeenCalledWith("xfyun", "api_key", "secret-value");
    expect(input.value).toBe("");
  });

  it("切换服务商套用默认模型并通知外部", () => {
    const h = setup();
    fireEvent.click(screen.getByText("腾讯云语音识别"));

    expect(h.onProviderChange).toHaveBeenCalledWith("tencent");
    expect((screen.getByLabelText("云端模型") as HTMLInputElement).value).toBe("16k_zh");
    expect(screen.getByLabelText("SecretId")).toBeTruthy();
  });

  it("清空凭证按当前服务商触发", () => {
    const h = setup();
    fireEvent.click(screen.getByText("清空该服务商全部凭证"));
    expect(h.onDeleteCredentials).toHaveBeenCalledWith("xfyun");
  });

  it("配置非法时禁用保存", () => {
    setup();
    fireEvent.change(screen.getByLabelText("云端模型"), { target: { value: " " } });

    expect(screen.getByText("云端模型名不能为空")).toBeTruthy();
    const saveButton = screen.getByText("保存配置") as HTMLButtonElement;
    expect(saveButton.disabled).toBe(true);
  });
});
