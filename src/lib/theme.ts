/**
 * 主题纯逻辑（ST-13）
 *
 * 职责：把「设置里的主题值」解析为「实际生效主题」，并应用到 DOM。
 * 规则：
 * - `:root` 定义浅色令牌、`.dark` 定义深色令牌（见 `src/index.css`），本模块只负责切换 `html` 上的 class；
 * - `ei.theme` 本地缓存用于消除首屏闪烁（`index.html` 内联脚本与运行时共用），
 *   仅在持久化主题变化时写入，避免「未保存的预览」污染下次启动的首屏。
 */
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauri } from "./utils";

/** 实际生效的主题 */
export type ResolvedTheme = "dark" | "light";
/** 设置中可选的主题（含跟随系统） */
export type ThemeSetting = ResolvedTheme | "system";

/** 首屏主题缓存键（与 `ei.locale` 约定一致） */
export const THEME_STORAGE_KEY = "ei.theme";

/** 当前已应用到文档的主题（供需要感知主题的组件只读订阅） */
let appliedTheme: ResolvedTheme = loadThemeCache() ?? "dark";
const appliedThemeListeners = new Set<() => void>();

/** 读取当前生效主题（与 DOM 上 `dark` 类保持同步） */
export function getAppliedTheme(): ResolvedTheme {
  return appliedTheme;
}

/** 订阅当前生效主题的变化，返回取消订阅函数 */
export function subscribeAppliedTheme(listener: () => void): () => void {
  appliedThemeListeners.add(listener);
  return () => {
    appliedThemeListeners.delete(listener);
  };
}

/** 把设置值解析为实际主题；`system` 需由调用方提供当前系统主题 */
export function resolveTheme(
  setting: ThemeSetting,
  systemTheme: ResolvedTheme
): ResolvedTheme {
  return setting === "system" ? systemTheme : setting;
}

/**
 * 应用主题到文档：切换 `html` 的 `dark` 类、同步 `color-scheme`，
 * 并在 `persist` 为 true 时写入首屏缓存。
 */
export function applyTheme(theme: ResolvedTheme, persist = true): void {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  root.classList.toggle("dark", theme === "dark");
  // color-scheme 会同时影响 WebView 原生控件（滚动条、下拉、表单）的渲染
  root.style.colorScheme = theme;
  const meta = document.querySelector('meta[name="color-scheme"]');
  meta?.setAttribute("content", theme === "dark" ? "dark light" : "light dark");
  if (persist) saveThemeCache(theme);

  if (appliedTheme !== theme) {
    appliedTheme = theme;
    appliedThemeListeners.forEach((listener) => listener());
  }
}

/** 读取首屏主题缓存（供 `index.html` 内联脚本与运行时共用） */
export function loadThemeCache(): ResolvedTheme | null {
  if (typeof localStorage === "undefined") return null;
  try {
    const cached = localStorage.getItem(THEME_STORAGE_KEY);
    return cached === "dark" || cached === "light" ? cached : null;
  } catch (err) {
    console.error("读取主题缓存失败:", err);
    return null;
  }
}

/** 写入首屏主题缓存 */
export function saveThemeCache(theme: ResolvedTheme): void {
  if (typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(THEME_STORAGE_KEY, theme);
  } catch (err) {
    console.error("写入主题缓存失败:", err);
  }
}

/** 同步兜底：基于 `prefers-color-scheme` 判定系统主题（非 Tauri 环境的主路径） */
export function systemThemeFromMediaQuery(): ResolvedTheme {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return "dark";
  }
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

/**
 * 读取系统主题：Tauri 下以原生窗口主题为准（更贴近操作系统真实设置），
 * 非 Tauri 或读取失败时回落 `prefers-color-scheme`。
 */
export async function getSystemTheme(): Promise<ResolvedTheme> {
  if (isTauri()) {
    try {
      const theme = await getCurrentWindow().theme();
      if (theme === "dark" || theme === "light") return theme;
    } catch (err) {
      console.error("读取系统主题失败:", err);
    }
  }
  return systemThemeFromMediaQuery();
}

/**
 * 订阅系统主题变化（仅在「跟随系统」模式下调用）。
 * 返回同步的取消订阅函数；Tauri 监听为异步注册，注册完成前取消也能正确清理。
 */
export function subscribeSystemTheme(
  onChange: (theme: ResolvedTheme) => void
): () => void {
  let disposed = false;
  let unlisten: (() => void) | null = null;

  if (isTauri()) {
    getCurrentWindow()
      .onThemeChanged((event) => {
        const theme = event.payload;
        if (theme === "dark" || theme === "light") onChange(theme);
      })
      .then((un) => {
        if (disposed) un();
        else unlisten = un;
      })
      .catch((err) => console.error("监听系统主题失败:", err));
  } else if (typeof window !== "undefined" && typeof window.matchMedia === "function") {
    const query = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = (event: MediaQueryListEvent) =>
      onChange(event.matches ? "dark" : "light");
    query.addEventListener("change", handler);
    unlisten = () => query.removeEventListener("change", handler);
  }

  return () => {
    disposed = true;
    unlisten?.();
    unlisten = null;
  };
}
