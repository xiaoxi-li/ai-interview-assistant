import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import AsrSettingsSection from "../AsrSettingsSection";
import type {
  ASRConfig,
  AsrCredentialStatus,
  AsrEngineInfo,
  AsrProviderPreset,
} from "../../../types";

const providers: AsrProviderPreset[] = [
  {
    provider: "aliyun",
    label: "阿里云智能语音",
    defaultModel: "gummy-realtime-v1",
    credentialFields: ["api_key"],
    defaultBaseUrl: "wss://dashscope.aliyuncs.com/api-ws/v1/inference",
    implemented: true,
    usesWebsocket: true,
  },
  {
    provider: "xfyun",
    label: "讯飞开放平台",
    defaultModel: "iat",
    credentialFields: ["app_id", "api_key", "api_secret"],
    defaultBaseUrl: "wss://ws-api.xfyun.cn/v2/iat",
    implemented: false,
    usesWebsocket: true,
  },
  {
    provider: "tencent",
    label: "腾讯云语音识别",
    defaultModel: "16k_zh",
    credentialFields: ["secret_id", "secret_key"],
    defaultBaseUrl: "wss://asr.cloud.tencent.com/asr/v2",
    implemented: false,
    usesWebsocket: true,
  },
];

const config: ASRConfig = {
  mode: "cloud",
  cloudProvider: "xfyun",
  model: "iat",
  language: "zh-CN",
  vadThreshold: 0.5,
  localModel: "sherpa-onnx-paraformer-zh",
  cloudBaseUrls: {},
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

const readyEngine: AsrEngineInfo = {
  id: "asr-paraformer-zh",
  label: "本地 ASR（sherpa-onnx 中文 paraformer）",
  ready: true,
  detail: "模型已就绪（C:/models/asr-paraformer-zh）；本地识别已接入",
};

const blockedEngine: AsrEngineInfo = {
  ...readyEngine,
  ready: false,
  detail:
    "模型已就绪（C:/models/asr-paraformer-zh）；本地识别代码已就绪，但本次构建未启用（需构建机安装 libclang 后以 --features local-asr-native 构建，见 README）",
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

  it("Base URL 初始为空并以官方默认为占位，恢复默认按钮禁用", () => {
    setup();
    const input = screen.getByLabelText("服务端地址（Base URL）") as HTMLInputElement;
    expect(input.value).toBe("");
    expect(input.placeholder).toBe("wss://ws-api.xfyun.cn/v2/iat");
    expect((screen.getByText("恢复默认") as HTMLButtonElement).disabled).toBe(true);
    // 生效地址展示官方默认
    expect(screen.getByText(/当前生效地址：wss:\/\/ws-api\.xfyun\.cn\/v2\/iat/)).toBeTruthy();
  });

  it("每个服务商各存一份 Base URL，切换服务商后互不覆盖", () => {
    setup();

    // 阿里云填自定义地址
    fireEvent.click(screen.getByText("阿里云智能语音"));
    fireEvent.change(screen.getByLabelText("服务端地址（Base URL）"), {
      target: { value: "wss://proxy.example.com/dashscope" },
    });

    // 切到腾讯：应展示腾讯自己的默认（空输入 + 占位）
    fireEvent.click(screen.getByText("腾讯云语音识别"));
    const tencentInput = screen.getByLabelText("服务端地址（Base URL）") as HTMLInputElement;
    expect(tencentInput.value).toBe("");

    // 切回阿里：自定义值仍在
    fireEvent.click(screen.getByText("阿里云智能语音"));
    expect(
      (screen.getByLabelText("服务端地址（Base URL）") as HTMLInputElement).value
    ).toBe("wss://proxy.example.com/dashscope");
  });

  it("恢复默认清空当前服务商的自定义地址", () => {
    setup();
    fireEvent.click(screen.getByText("阿里云智能语音"));
    fireEvent.change(screen.getByLabelText("服务端地址（Base URL）"), {
      target: { value: "wss://proxy.example.com/dashscope" },
    });
    fireEvent.click(screen.getByText("恢复默认"));

    const input = screen.getByLabelText("服务端地址（Base URL）") as HTMLInputElement;
    expect(input.value).toBe("");
    expect((screen.getByText("恢复默认") as HTMLButtonElement).disabled).toBe(true);
  });

  it("Base URL 非法时给出提示并禁用保存", () => {
    setup();
    fireEvent.change(screen.getByLabelText("服务端地址（Base URL）"), {
      target: { value: "ftp://example.com" },
    });

    expect(screen.getByText(/服务端地址需以 ws:\/\//)).toBeTruthy();
    const saveButton = screen.getByText("保存配置") as HTMLButtonElement;
    expect(saveButton.disabled).toBe(true);
  });

  it("阿里云提示实时模型示例（避免误填非实时 HTTP 模型）", () => {
    setup();
    fireEvent.click(screen.getByText("阿里云智能语音"));

    expect(
      screen.getByText(/实时模型示例：qwen-audio-3\.1-asr-flash-streaming/)
    ).toBeTruthy();
    expect(screen.getByText(/不能用于实时转写/)).toBeTruthy();
  });

  it("Base URL 填成 HTTP 接口路径时给出防呆提示", () => {
    setup();
    fireEvent.change(screen.getByLabelText("服务端地址（Base URL）"), {
      target: { value: "https://dashscope.aliyuncs.com/compatible-mode/v1" },
    });

    expect(screen.getByText(/该路径属于 HTTP 接口/)).toBeTruthy();
  });

  it("标注服务商接入状态：已接入 / 待接入", () => {
    setup();
    expect(screen.getByText("已接入")).toBeTruthy();
    expect(screen.getAllByText("待接入").length).toBeGreaterThanOrEqual(2);
  });

  it("选中未接入服务商时给出待接入提示", () => {
    setup();
    expect(screen.getByText(/「讯飞开放平台」协议待接入/)).toBeTruthy();
  });

  it("保存配置回传含各服务商 Base URL 的草稿", () => {
    const h = setup();
    fireEvent.change(screen.getByLabelText("服务端地址（Base URL）"), {
      target: { value: "wss://proxy.example.com/iat" },
    });
    fireEvent.click(screen.getByText("保存配置"));

    expect(h.onSaveConfig).toHaveBeenCalledWith(
      expect.objectContaining({
        cloudBaseUrls: { xfyun: "wss://proxy.example.com/iat" },
      })
    );
  });

  it("云端模式提供连接测试并展示结果（IA-16.8）", () => {
    const onTestConnection = vi.fn();
    setup({
      onTestConnection,
      testResult: { ok: true, latencyMs: 128, message: "连接成功（128ms）" },
    });

    expect(screen.getByText("连接成功（128ms）")).toBeTruthy();
    fireEvent.click(screen.getByText("连接测试"));
    // 回传当前表单草稿：避免测到未保存的旧配置
    expect(onTestConnection).toHaveBeenCalledWith(
      expect.objectContaining({ mode: "cloud", cloudProvider: "xfyun" })
    );
  });

  it("连接测试进行中时按钮禁用", () => {
    setup({ testing: true });
    const button = screen.getByText("测试中…") as HTMLButtonElement;
    expect(button.disabled).toBe(true);
  });

  it("本地模式不展示连接测试", () => {
    setup();
    fireEvent.click(screen.getByText("本地（隐私优先）"));
    expect(screen.queryByText("连接测试")).toBeNull();
  });

  it("本地模式展示已就绪的本地引擎状态（IA-15）", () => {
    setup({ localEngine: readyEngine });
    fireEvent.click(screen.getByText("本地（隐私优先）"));

    expect(screen.getByText("本地引擎")).toBeTruthy();
    expect(screen.getByText("已就绪")).toBeTruthy();
    expect(screen.getByText(/本地识别已接入/)).toBeTruthy();
  });

  it("本地模式未启用原生推理时展示未启用与构建指引", () => {
    setup({ localEngine: blockedEngine });
    fireEvent.click(screen.getByText("本地（隐私优先）"));

    expect(screen.getByText("未启用")).toBeTruthy();
    expect(screen.getByText(/--features local-asr-native/)).toBeTruthy();
  });

  it("云端模式不展示本地引擎状态", () => {
    setup({ localEngine: readyEngine });
    expect(screen.queryByText("本地引擎")).toBeNull();
  });
});
