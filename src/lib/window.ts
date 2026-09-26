import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauri } from "./utils";
import { ipc } from "./ipc";

/** 迷你浮窗标签（与后端 MINI_WINDOW_LABEL 一致） */
export const MINI_WINDOW_LABEL = "mini";

/** 当前窗口标签；非 Tauri 环境返回 null */
export function currentWindowLabel(): string | null {
  if (!isTauri()) return null;
  try {
    return getCurrentWindow().label;
  } catch (err) {
    console.error("读取窗口标签失败:", err);
    return null;
  }
}

/** 是否运行在迷你浮窗中 */
export function isMiniWindow(): boolean {
  return currentWindowLabel() === MINI_WINDOW_LABEL;
}

/** 打开迷你浮窗（幂等，已存在则聚焦） */
export function openMiniWindow() {
  return ipc<void>("open_mini_window");
}

/** 关闭迷你浮窗 */
export function closeMiniWindow() {
  return ipc<void>("close_mini_window");
}
