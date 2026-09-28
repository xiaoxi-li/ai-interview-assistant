import { describe, expect, it } from "vitest";
import {
  isAiConfigDirty,
  platformLabel,
  thinkingParamHint,
  validateAiConfig,
} from "../aiConfig";
import type { AIConfig, PlatformPreset } from "../../types";

const base: AIConfig = {
  platform: "deepseek",
  baseUrl: "https://api.deepseek.com/v1",
  model: "deepseek-chat",
  stream: true,
  temperature: 0.7,
  thinking: "nonThinking",
};

describe("validateAiConfig", () => {
  it("合法配置通过校验", () => {
    expect(validateAiConfig(base)).toBeNull();
  });

  it("Base URL 为空时报错", () => {
    expect(validateAiConfig({ ...base, baseUrl: "  " })).toBe("Base URL 不能为空");
  });

  it("Base URL 缺少协议时报错", () => {
    expect(validateAiConfig({ ...base, baseUrl: "api.deepseek.com" })).toContain(
      "http"
    );
  });

  it("模型名为空时报错", () => {
    expect(validateAiConfig({ ...base, model: "" })).toBe("模型名不能为空");
  });

  it("温度越界时报错，边界值合法", () => {
    expect(validateAiConfig({ ...base, temperature: -0.1 })).toContain("温度");
    expect(validateAiConfig({ ...base, temperature: 2.1 })).toContain("温度");
    expect(validateAiConfig({ ...base, temperature: 0 })).toBeNull();
    expect(validateAiConfig({ ...base, temperature: 2 })).toBeNull();
  });
});

describe("isAiConfigDirty", () => {
  it("无已保存配置时视为脏", () => {
    expect(isAiConfigDirty(base, null)).toBe(true);
  });

  it("字段一致时为干净", () => {
    expect(isAiConfigDirty(base, { ...base })).toBe(false);
  });

  it("任一字段变化即为脏", () => {
    expect(isAiConfigDirty({ ...base, model: "x" }, base)).toBe(true);
    expect(isAiConfigDirty({ ...base, stream: false }, base)).toBe(true);
    expect(isAiConfigDirty({ ...base, temperature: 1 }, base)).toBe(true);
    expect(isAiConfigDirty({ ...base, thinking: "thinking" }, base)).toBe(true);
  });
});

describe("thinkingParamHint（ST-12）", () => {
  const preset = (thinkingParam: string | null): PlatformPreset => ({
    platform: "qwen",
    label: "通义千问 Qwen",
    defaultBaseUrl: "https://dashscope.aliyuncs.com/compatible-mode/v1",
    defaultModel: "qwen-plus",
    thinkingParam,
  });

  it("顶层布尔参数平台说明 enable_thinking", () => {
    expect(thinkingParamHint(preset("enable_thinking"))).toContain(
      "已按 enable_thinking 下发"
    );
  });

  it("嵌套参数平台说明 thinking.type", () => {
    expect(thinkingParamHint(preset("thinking"))).toContain(
      "已按 thinking.type 下发"
    );
  });

  it("无开关参数的平台提示改用模型名", () => {
    expect(thinkingParamHint(preset(null))).toContain("没有思考开关参数");
    expect(thinkingParamHint(undefined)).toContain("没有思考开关参数");
  });
});

describe("platformLabel", () => {
  const presets: PlatformPreset[] = [
    {
      platform: "glm",
      label: "智谱 GLM",
      defaultBaseUrl: "https://open.bigmodel.cn/api/paas/v4",
      defaultModel: "glm-4-plus",
      thinkingParam: "thinking",
    },
  ];

  it("命中预设时返回中文标签", () => {
    expect(platformLabel("glm", presets)).toBe("智谱 GLM");
  });

  it("未命中时回退到平台标识", () => {
    expect(platformLabel("qwen", presets)).toBe("qwen");
  });
});
