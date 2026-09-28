import { afterEach, describe, expect, it, vi } from "vitest";
import {
  applyTheme,
  getAppliedTheme,
  loadThemeCache,
  resolveTheme,
  subscribeAppliedTheme,
  systemThemeFromMediaQuery,
} from "../theme";

describe("resolveTheme", () => {
  it("深色 / 浅色直接生效", () => {
    expect(resolveTheme("dark", "light")).toBe("dark");
    expect(resolveTheme("light", "dark")).toBe("light");
  });

  it("跟随系统时取系统主题", () => {
    expect(resolveTheme("system", "light")).toBe("light");
    expect(resolveTheme("system", "dark")).toBe("dark");
  });
});

describe("applyTheme", () => {
  afterEach(() => {
    document.documentElement.classList.remove("dark");
    document.documentElement.style.colorScheme = "";
    localStorage.clear();
    applyTheme("dark", false);
  });

  it("切换 html 上的 dark 类与 color-scheme", () => {
    applyTheme("light", false);
    expect(document.documentElement.classList.contains("dark")).toBe(false);
    expect(document.documentElement.style.colorScheme).toBe("light");
    expect(getAppliedTheme()).toBe("light");

    applyTheme("dark", false);
    expect(document.documentElement.classList.contains("dark")).toBe(true);
    expect(document.documentElement.style.colorScheme).toBe("dark");
    expect(getAppliedTheme()).toBe("dark");
  });

  it("persist 为 true 时写首屏缓存，false 时不写（预览不污染缓存）", () => {
    applyTheme("light", true);
    expect(loadThemeCache()).toBe("light");

    applyTheme("dark", false);
    expect(loadThemeCache()).toBe("light");
  });

  it("主题变化时通知订阅者，重复设置同一主题不再通知", () => {
    applyTheme("dark", false);
    const listener = vi.fn();
    const unsubscribe = subscribeAppliedTheme(listener);

    applyTheme("light", false);
    expect(listener).toHaveBeenCalledTimes(1);

    applyTheme("light", false);
    expect(listener).toHaveBeenCalledTimes(1);

    unsubscribe();
    applyTheme("dark", false);
    expect(listener).toHaveBeenCalledTimes(1);
  });
});

describe("loadThemeCache", () => {
  afterEach(() => localStorage.clear());

  it("只接受 dark / light，非法值返回 null", () => {
    expect(loadThemeCache()).toBeNull();

    localStorage.setItem("ei.theme", "light");
    expect(loadThemeCache()).toBe("light");

    localStorage.setItem("ei.theme", "system");
    expect(loadThemeCache()).toBeNull();
  });
});

describe("systemThemeFromMediaQuery", () => {
  it("始终返回 dark 或 light", () => {
    expect(["dark", "light"]).toContain(systemThemeFromMediaQuery());
  });
});
