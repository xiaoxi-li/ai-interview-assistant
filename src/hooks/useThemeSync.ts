/**
 * 主题同步（ST-13）：把设置中的主题值落到界面上的唯一副作用入口。
 *
 * 解析优先级：`previewTheme`（设置页即时预览）→ 已保存 `interfaceConfig.theme`
 * → 首屏缓存 → 默认深色。`system` 模式下实时跟随操作系统主题变化。
 * 必须在 `App` 顶层调用（迷你模式与向导早退之前），否则这些界面不受主题控制。
 */
import { useEffect, useState, useSyncExternalStore } from "react";
import {
  applyTheme,
  getAppliedTheme,
  getSystemTheme,
  loadThemeCache,
  resolveTheme,
  subscribeAppliedTheme,
  subscribeSystemTheme,
  systemThemeFromMediaQuery,
  type ResolvedTheme,
} from "../lib/theme";
import { useSettingsStore } from "../stores/settingsStore";

/**
 * 只读当前生效主题（不产生副作用，可多处使用）。
 * 需要感知主题的组件（如代码编辑器）用它即可，无需重复调用 `useThemeSync`。
 */
export function useResolvedTheme(): ResolvedTheme {
  return useSyncExternalStore(subscribeAppliedTheme, getAppliedTheme, getAppliedTheme);
}

/** 返回当前实际生效的主题，供需要感知主题的组件（如代码编辑器）使用 */
export function useThemeSync(): ResolvedTheme {
  const previewTheme = useSettingsStore((s) => s.previewTheme);
  const savedTheme = useSettingsStore((s) => s.interfaceConfig?.theme);

  const isPreview = previewTheme != null;
  // 配置未加载完成时用首屏缓存兜底，避免"先按缓存渲染、再跳回默认深色"
  const setting = previewTheme ?? savedTheme ?? loadThemeCache() ?? "dark";

  const [systemTheme, setSystemTheme] = useState<ResolvedTheme>(() =>
    systemThemeFromMediaQuery()
  );

  // 读取真实系统主题（Tauri 窗口主题优先于 media query）
  useEffect(() => {
    let alive = true;
    getSystemTheme().then((theme) => {
      if (alive) setSystemTheme(theme);
    });
    return () => {
      alive = false;
    };
  }, []);

  // 仅在「跟随系统」时监听系统主题变化，其他模式不注册监听
  useEffect(() => {
    if (setting !== "system") return;
    return subscribeSystemTheme(setSystemTheme);
  }, [setting]);

  const resolved = resolveTheme(setting, systemTheme);

  useEffect(() => {
    // 预览态不写首屏缓存：缓存始终代表"已持久化的主题"
    applyTheme(resolved, !isPreview);
  }, [resolved, isPreview]);

  return resolved;
}
