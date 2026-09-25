import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { isTauri } from "./utils";
import { mockInvoke } from "../mocks/handlers";

/**
 * 统一 IPC 调用入口。
 *
 * 运行在 Tauri 中时调用 Rust 命令；在浏览器中（vite dev 独立调试前端）时
 * 自动降级到 `src/mocks/handlers.ts` 的 mock 实现，保证前端可脱离桌面壳开发。
 */
export async function ipc<T>(
  command: string,
  args?: Record<string, unknown>
): Promise<T> {
  if (isTauri()) {
    return tauriInvoke<T>(command, args);
  }
  return mockInvoke<T>(command, args);
}
