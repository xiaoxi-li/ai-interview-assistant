import { describe, expect, it } from "vitest";
import {
  activeAsrModel,
  credentialFieldLabel,
  effectiveBaseUrl,
  hasCustomBaseUrl,
  missingCredentials,
  validateAsrConfig,
  validateBaseUrl,
  withProviderBaseUrl,
} from "../asrConfig";
import type { ASRConfig } from "../../types";

const base: ASRConfig = {
  mode: "cloud",
  cloudProvider: "xfyun",
  model: "iat",
  language: "zh-CN",
  vadThreshold: 0.5,
  localModel: "sherpa-onnx-paraformer-zh",
};

describe("validateAsrConfig", () => {
  it("合法配置通过", () => {
    expect(validateAsrConfig(base)).toBeNull();
  });

  it("模型名为空时报错", () => {
    expect(validateAsrConfig({ ...base, model: " " })).toBe("云端模型名不能为空");
    expect(validateAsrConfig({ ...base, localModel: "" })).toBe(
      "本地模型名不能为空"
    );
  });

  it("语言为空时报错", () => {
    expect(validateAsrConfig({ ...base, language: "" })).toBe("识别语言不能为空");
  });

  it("VAD 阈值越界时报错，边界合法", () => {
    expect(validateAsrConfig({ ...base, vadThreshold: -0.2 })).toContain("VAD");
    expect(validateAsrConfig({ ...base, vadThreshold: 1.4 })).toContain("VAD");
    expect(validateAsrConfig({ ...base, vadThreshold: 0 })).toBeNull();
    expect(validateAsrConfig({ ...base, vadThreshold: 1 })).toBeNull();
  });
});

describe("activeAsrModel", () => {
  it("云端模式使用云端模型", () => {
    expect(activeAsrModel(base)).toBe("iat");
  });

  it("本地模式使用本地模型", () => {
    expect(activeAsrModel({ ...base, mode: "local" })).toBe(
      "sherpa-onnx-paraformer-zh"
    );
  });
});

describe("missingCredentials", () => {
  it("云端模式返回缺失字段", () => {
    expect(missingCredentials(base, ["app_id"])).toEqual([
      "api_key",
      "api_secret",
    ]);
  });

  it("全部配置后无缺失", () => {
    expect(
      missingCredentials(base, ["app_id", "api_key", "api_secret"])
    ).toEqual([]);
  });

  it("本地模式不需要凭证", () => {
    expect(missingCredentials({ ...base, mode: "local" }, [])).toEqual([]);
  });
});

describe("credentialFieldLabel", () => {
  it("映射常见字段显示名", () => {
    expect(credentialFieldLabel("app_id")).toBe("AppID");
    expect(credentialFieldLabel("api_secret")).toBe("API Secret");
  });

  it("未知字段回退原值", () => {
    expect(credentialFieldLabel("custom_field")).toBe("custom_field");
  });
});

describe("Base URL 校验与解析", () => {
  it("空值视为合法（使用官方默认）", () => {
    expect(validateBaseUrl("")).toBeNull();
    expect(validateBaseUrl(undefined)).toBeNull();
    expect(validateBaseUrl("   ")).toBeNull();
  });

  it("接受 ws(s)/http(s)，拒绝其他 scheme", () => {
    expect(validateBaseUrl("wss://example.com/ws")).toBeNull();
    expect(validateBaseUrl("ws://example.com/ws")).toBeNull();
    expect(validateBaseUrl("https://example.com/v1")).toBeNull();
    expect(validateBaseUrl("http://example.com/v1")).toBeNull();
    expect(validateBaseUrl("ftp://example.com")).toContain("ws://");
  });

  it("配置校验会把非法地址定位到具体服务商", () => {
    const config: ASRConfig = {
      ...base,
      cloudBaseUrls: { aliyun: "ftp://example.com" },
    };
    const error = validateAsrConfig(config);
    expect(error).toContain("阿里云智能语音");
    expect(error).toContain("ws://");
  });

  it("生效地址优先自定义值，否则回退官方默认", () => {
    expect(effectiveBaseUrl(base, "aliyun", "wss://default")).toBe("wss://default");
    const custom: ASRConfig = {
      ...base,
      cloudBaseUrls: { aliyun: " wss://proxy.example.com/ws " },
    };
    expect(effectiveBaseUrl(custom, "aliyun", "wss://default")).toBe(
      "wss://proxy.example.com/ws"
    );
    expect(hasCustomBaseUrl(custom, "aliyun")).toBe(true);
    expect(hasCustomBaseUrl(custom, "tencent")).toBe(false);
  });

  it("每个服务商各存一份，空白值等价于恢复默认", () => {
    let config = withProviderBaseUrl(base, "aliyun", "wss://a.example.com");
    config = withProviderBaseUrl(config, "tencent", "wss://t.example.com");
    expect(config.cloudBaseUrls).toEqual({
      aliyun: "wss://a.example.com",
      tencent: "wss://t.example.com",
    });

    config = withProviderBaseUrl(config, "aliyun", "   ");
    expect(config.cloudBaseUrls).toEqual({ tencent: "wss://t.example.com" });
  });
});
