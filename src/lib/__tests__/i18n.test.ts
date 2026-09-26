import { beforeEach, describe, expect, it } from "vitest";
import {
  DEFAULT_LOCALE,
  LOCALES,
  englishEntryCount,
  loadLocale,
  saveLocale,
  translate,
} from "../i18n";

describe("i18n", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  it("中文环境返回原文", () => {
    expect(translate("zh-CN", "开始转写")).toBe("开始转写");
    expect(translate("zh-CN", "尚未翻译的文案")).toBe("尚未翻译的文案");
  });

  it("英文环境返回词典文案", () => {
    expect(translate("en-US", "开始转写")).toBe("Start transcription");
    expect(translate("en-US", "知识库")).toBe("Knowledge");
  });

  it("英文缺失时回落中文原文（不出现空串或 key）", () => {
    const key = "这段文案尚未翻译";
    expect(translate("en-US", key)).toBe(key);
  });

  it("支持 {name} 占位符插值", () => {
    expect(translate("en-US", "已识别 {count} 个问题", { count: 3 })).toBe(
      "3 questions detected"
    );
    expect(translate("zh-CN", "已识别 {count} 个问题", { count: 3 })).toBe(
      "已识别 3 个问题"
    );
    expect(translate("en-US", "共 {count} 场", { count: 12 })).toBe("12 sessions");
  });

  it("缺少占位符参数时保留占位符原样", () => {
    expect(translate("en-US", "已识别 {count} 个问题")).toBe(
      "{count} questions detected"
    );
    expect(translate("zh-CN", "已识别 {count} 个问题", { other: 1 })).toBe(
      "已识别 {count} 个问题"
    );
  });

  it("语言选择可持久化", () => {
    expect(loadLocale()).toBe(DEFAULT_LOCALE);

    saveLocale("en-US");
    expect(loadLocale()).toBe("en-US");

    // 非法值回退默认
    window.localStorage.setItem("aia.locale", "fr-FR");
    expect(loadLocale()).toBe(DEFAULT_LOCALE);
  });

  it("语言列表包含中英两种", () => {
    expect(LOCALES.map((item) => item.value)).toEqual(["zh-CN", "en-US"]);
  });

  it("英文词典已覆盖核心界面文案", () => {
    expect(englishEntryCount()).toBeGreaterThan(60);
    for (const key of ["开始转写", "停止转写", "一键提问", "AI 回答", "会话历史", "设置"]) {
      expect(translate("en-US", key)).not.toBe(key);
    }
  });
});
