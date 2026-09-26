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

    case "get_interface_config":
      return {
        theme: "dark",
        hotkeys: [
          { action: "ask", accelerator: "Ctrl+Shift+A" },
          { action: "regenerate", accelerator: "Ctrl+Shift+R" },
          { action: "copy_answer", accelerator: "Ctrl+Shift+C" },
          { action: "pause_capture", accelerator: "Ctrl+Shift+P" },
          { action: "toggle_mini", accelerator: "Ctrl+Shift+M" },
        ],
        autoStart: false,
        trayBehavior: "minimize",
      } as T;

    case "save_interface_config":
      return (args?.config ?? null) as T;

    case "get_autostart":
      return false as T;

    case "set_autostart":
      return Boolean(args?.enabled) as T;

    case "get_storage_paths":
      return {
        dbPath: "C:\\Users\\demo\\AppData\\Roaming\\ai-interview\\ai-interview.db",
        modelsDir: "C:\\Users\\demo\\AppData\\Roaming\\ai-interview\\models",
      } as T;

    case "get_knowledge_config":
      return {
        dbPath: null,
        modelsDir: null,
        embeddingModel: "embedding-minilm-zh",
      } as T;

    case "save_knowledge_config":
      return (args?.config ?? null) as T;

    case "list_models":
      return [
        {
          id: "embedding-minilm-zh",
          name: "中文嵌入模型（MiniLM）",
          kind: "embedding",
          fileName: "embedding-minilm-zh.onnx",
          path: "embedding/minilm-zh.onnx",
          sha256: "",
          sizeBytes: 94371840,
          description: "本地向量检索使用的轻量嵌入模型，约 90MB",
          requiredForLocalAsr: false,
          installed: true,
          resolvedPath: "C:\\Users\\demo\\AppData\\Roaming\\ai\\models\\embedding\\minilm-zh.onnx",
          partialBytes: 0,
        },
        {
          id: "vad-silero",
          name: "Silero VAD",
          kind: "vad",
          fileName: "silero_vad.onnx",
          path: "vad/silero_vad.onnx",
          sha256: "",
          sizeBytes: 2097152,
          description: "语音活动检测模型，用于静音过滤与自动分段，约 2MB",
          requiredForLocalAsr: false,
          installed: false,
          resolvedPath: null,
          partialBytes: 524288,
        },
        {
          id: "asr-paraformer-zh",
          name: "本地 ASR（Paraformer 中文）",
          kind: "asr",
          fileName: "sherpa-onnx-paraformer-zh.tar.bz2",
          path: "asr/sherpa-onnx-paraformer-zh.tar.bz2",
          sha256: "",
          sizeBytes: 209715200,
          description: "本地语音识别模型，仅在选择本地 ASR 模式时需要，约 200MB",
          requiredForLocalAsr: true,
          installed: false,
          resolvedPath: null,
          partialBytes: 0,
        },
      ] as T;

    case "download_model":
      return "mock://models/downloaded" as T;

    case "delete_model":
      return true as T;

    case "get_privacy_config":
      return { uploadAudio: false, encryptLocal: false } as T;

    case "save_privacy_config":
      return (args?.config ?? null) as T;

    case "clear_local_data":
      return { notes: 0, groups: 0, tags: 0, sessions: 0 } as T;

    case "list_loopback_devices":
      return [
        {
          id: "立体声混音 (Realtek)",
          name: "立体声混音 (Realtek)",
          kind: "input",
          isDefault: false,
        },
      ] as T;

    case "audio_capture_status":
      return {
        running: false,
        source: null,
        micDevice: "",
        systemDevice: "",
        loopbackAvailable: true,
      } as T;

    case "start_audio_capture":
      return {
        running: true,
        source: String(args?.source ?? "mic"),
        micDevice: "麦克风 (USB Audio)",
        systemDevice: "立体声混音 (Realtek)",
        loopbackAvailable: true,
      } as T;

    case "stop_audio_capture":
      return {
        running: false,
        source: null,
        micDevice: "麦克风 (USB Audio)",
        systemDevice: "立体声混音 (Realtek)",
        loopbackAvailable: true,
      } as T;

    case "list_audio_devices":
      return [
        { id: "麦克风 (USB Audio)", name: "麦克风 (USB Audio)", kind: "input", isDefault: true },
        { id: "麦克风 (Realtek)", name: "麦克风 (Realtek)", kind: "input", isDefault: false },
        { id: "扬声器 (Realtek)", name: "扬声器 (Realtek)", kind: "output", isDefault: true },
        { id: "耳机 (Bluetooth)", name: "耳机 (Bluetooth)", kind: "output", isDefault: false },
      ] as T;

    case "get_audio_config":
      return { micDeviceId: null, systemDeviceId: null } as T;

    case "save_audio_config":
      return (args?.config ?? null) as T;

    case "list_asr_providers":
      return [
        {
          provider: "xfyun",
          label: "讯飞开放平台",
          defaultModel: "iat",
          credentialFields: ["app_id", "api_key", "api_secret"],
        },
        {
          provider: "aliyun",
          label: "阿里云智能语音",
          defaultModel: "gummy-realtime-v1",
          credentialFields: ["api_key"],
        },
        {
          provider: "tencent",
          label: "腾讯云语音识别",
          defaultModel: "16k_zh",
          credentialFields: ["secret_id", "secret_key"],
        },
      ] as T;

    case "get_asr_config":
      return {
        mode: "cloud",
        cloudProvider: "xfyun",
        model: "iat",
        language: "zh-CN",
        vadThreshold: 0.5,
        localModel: "sherpa-onnx-paraformer-zh",
      } as T;

    case "save_asr_config":
      return (args?.config ?? null) as T;

    case "get_asr_credential_status":
    case "save_asr_credential":
    case "delete_asr_credentials": {
      const provider = String(args?.provider ?? "xfyun");
      const clearing = command === "delete_asr_credentials";
      const fields =
        provider === "xfyun"
          ? ["app_id", "api_key", "api_secret"]
          : provider === "tencent"
            ? ["secret_id", "secret_key"]
            : ["api_key"];
      return {
        provider,
        complete: !clearing,
        fields: fields.map((field) => ({
          field,
          configured: !clearing,
          masked: clearing ? "" : "ab****yz",
        })),
      } as T;
    }

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
