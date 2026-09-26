import { describe, expect, it } from "vitest";
import {
  activeAsrModel,
  credentialFieldLabel,
  missingCredentials,
  validateAsrConfig,
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
