import { MOCK_GROUPS, MOCK_NOTES } from "./data";

/**
 * 非 Tauri 环境下的 IPC 降级实现。
 * 返回值形状需与 Rust 命令保持一致，便于前端独立开发与调试。
 */
export async function mockInvoke<T>(
  command: string,
  args?: Record<string, unknown>
): Promise<T> {
  switch (command) {
    case "app_info":
      return { name: "AI 面试助手", version: "0.1.0-mock" } as T;

    case "list_groups":
      return MOCK_GROUPS as T;

    case "list_notes":
      return MOCK_NOTES as T;

    case "search_notes": {
      const query = String(args?.query ?? "").trim().toLowerCase();
      if (!query) return [] as T;
      // 简化的前端 mock 检索：按标题/正文/标签包含匹配
      return MOCK_NOTES.filter(
        (n) =>
          n.title.toLowerCase().includes(query) ||
          n.content.toLowerCase().includes(query) ||
          n.tags.some((t) => t.toLowerCase().includes(query))
      ).map((n) => ({
        noteId: n.id,
        title: n.title,
        snippet: n.content.replace(/\s+/g, " ").slice(0, 80),
        score: 1,
      })) as T;
    }

    default:
      console.warn(`[mock] 未实现的命令：${command}`, args ?? {});
      throw new Error(`mock 未实现命令：${command}`);
  }
}
