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

    case "list_tags":
      return [
        { id: "t1", name: "React", color: "#4F6BED" },
        { id: "t2", name: "性能优化", color: "#4F6BED" },
        { id: "t3", name: "行为面试", color: "#4F6BED" },
      ] as T;

    case "list_note_versions":
      return [] as T;

    case "list_platform_presets":
      return [
        {
          platform: "deepseek",
          label: "DeepSeek",
          defaultBaseUrl: "https://api.deepseek.com/v1",
          defaultModel: "deepseek-chat",
        },
        {
          platform: "qwen",
          label: "通义千问 Qwen",
          defaultBaseUrl: "https://dashscope.aliyuncs.com/compatible-mode/v1",
          defaultModel: "qwen-plus",
        },
        {
          platform: "glm",
          label: "智谱 GLM",
          defaultBaseUrl: "https://open.bigmodel.cn/api/paas/v4",
          defaultModel: "glm-4-plus",
        },
        {
          platform: "chatgpt",
          label: "ChatGPT",
          defaultBaseUrl: "https://api.openai.com/v1",
          defaultModel: "gpt-4o-mini",
        },
      ] as T;

    case "get_ai_config":
      return {
        platform: "deepseek",
        baseUrl: "https://api.deepseek.com/v1",
        model: "deepseek-chat",
        stream: true,
        temperature: 0.7,
      } as T;

    case "save_ai_config":
      return (args?.config ?? null) as T;

    case "test_ai_connection":
      return {
        ok: true,
        latencyMs: 42,
        message: "连接成功（HTTP 200，mock 环境）",
      } as T;

    case "get_api_key_status":
    case "save_api_key":
    case "delete_api_key": {
      const platform = String(args?.platform ?? "unknown");
      const provided = String(args?.apiKey ?? "");
      const configured = command === "delete_api_key" ? false : provided.length > 0;
      return {
        platform,
        configured,
        masked: configured ? `${provided.slice(0, 3)}****${provided.slice(-4)}` : "",
      } as T;
    }

    case "set_note_tags": {
      const id = String(args?.id ?? "");
      const tags = (args?.tags as string[]) ?? [];
      const note = MOCK_NOTES.find((n) => n.id === id);
      return (note ? { ...note, tags } : null) as T;
    }

    case "list_notes":
      return MOCK_NOTES as T;

    case "search_hybrid_notes":
    case "search_similar_notes":
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
