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
      return { name: "轻松面试", version: "0.1.0-mock" } as T;

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
          thinkingParam: null,
        },
        {
          platform: "qwen",
          label: "通义千问 Qwen",
          defaultBaseUrl: "https://dashscope.aliyuncs.com/compatible-mode/v1",
          // ST-14：与后端 `AiPlatform::Qwen.preset()` 保持一致
          defaultModel: "qwen3.8-flash",
          thinkingParam: "enable_thinking",
        },
        {
          platform: "glm",
          label: "智谱 GLM",
          defaultBaseUrl: "https://open.bigmodel.cn/api/paas/v4",
          defaultModel: "glm-4-plus",
          thinkingParam: "thinking",
        },
        {
          platform: "siliconflow",
          label: "硅基流动 SiliconFlow",
          defaultBaseUrl: "https://api.siliconflow.cn/v1",
          defaultModel: "deepseek-ai/DeepSeek-V3",
          thinkingParam: "enable_thinking",
        },
        {
          platform: "openai",
          label: "OpenAI",
          defaultBaseUrl: "https://api.openai.com/v1",
          defaultModel: "gpt-4o-mini",
          thinkingParam: null,
        },
      ] as T;

    case "get_ai_config":
      // ST-14：与后端 `AiConfig::default()`（Qwen + qwen3.8-flash）保持一致
      return {
        platform: "qwen",
        baseUrl: "https://dashscope.aliyuncs.com/compatible-mode/v1",
        model: "qwen3.8-flash",
        stream: true,
        temperature: 0.7,
        thinking: "nonThinking",
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
        dbPath: "C:\\Users\\demo\\AppData\\Roaming\\com.easyinterview.app\\easy-interview.db",
        modelsDir: "C:\\Users\\demo\\AppData\\Roaming\\com.easyinterview.app\\models",
      } as T;

    case "get_knowledge_config":
      return {
        dbPath: null,
        modelsDir: null,
        embeddingModel: "bge-m3",
      } as T;

    case "save_knowledge_config":
      return (args?.config ?? null) as T;

    case "list_models":
      return [
        {
          id: "bge-m3",
          name: "嵌入模型（BGE-M3）",
          kind: "embedding",
          repo: "BAAI/bge-m3",
          files: [
            { remotePath: "onnx/model.onnx", localName: "model.onnx", sha256: "", sizeBytes: 724923 },
            { remotePath: "onnx/model.onnx_data", localName: "model.onnx_data", sha256: "", sizeBytes: 2266820608 },
            { remotePath: "onnx/tokenizer.json", localName: "tokenizer.json", sha256: "", sizeBytes: 17082821 },
            { remotePath: "onnx/config.json", localName: "config.json", sha256: "", sizeBytes: 698 },
          ],
          description: "本地向量检索使用的多语言嵌入模型（1024 维），约 2.1GB",
          requiredForLocalAsr: false,
          installed: true,
          resolvedPath: "C:\\Users\\demo\\AppData\\Roaming\\ai\\models\\bge-m3",
          partialBytes: 0,
        },
        {
          id: "vad-silero",
          name: "Silero VAD",
          kind: "vad",
          repo: "pengzhendong/silero-vad",
          files: [
            { remotePath: "silero_vad.onnx", localName: "silero_vad.onnx", sha256: "", sizeBytes: 2327524 },
          ],
          description: "语音活动检测模型，用于静音过滤与自动分段，约 2.3MB",
          requiredForLocalAsr: false,
          installed: false,
          resolvedPath: null,
          partialBytes: 524288,
        },
        {
          id: "asr-paraformer-zh",
          name: "本地 ASR（Paraformer 中文）",
          kind: "asr",
          repo: "pengzhendong/sherpa-onnx-paraformer-zh",
          files: [
            { remotePath: "model.int8.onnx", localName: "model.int8.onnx", sha256: "90bc03034ae1bef9575f8cc798cd1519c8be8aa9e8b458a033e32017ff4d584c", sizeBytes: 227330205 },
            { remotePath: "tokens.txt", localName: "tokens.txt", sha256: "6c0e3b35cece259829e6cb5b8d90d13db88f61ea3a2953d11898e4b2bfd7a2e2", sizeBytes: 75354 },
            { remotePath: "configuration.json", localName: "configuration.json", sha256: "", sizeBytes: 56 },
          ],
          description: "本地语音识别模型，仅在选择本地 ASR 模式时需要，约 217MB",
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
      // ST-14：与后端 `PrivacyConfig::default()`（默认允许上传）保持一致
      return { uploadAudio: true, encryptLocal: false } as T;

    case "save_privacy_config":
      return (args?.config ?? null) as T;

    case "clear_local_data":
      return { notes: 0, groups: 0, tags: 0, sessions: 0 } as T;

    // 系统音频来源：Windows 为播放设备（IA-14 端点级回环）
    case "list_loopback_devices":
      return [
        {
          id: "扬声器 (Realtek)",
          name: "扬声器 (Realtek)",
          kind: "output",
          isDefault: true,
        },
        {
          id: "耳机 (Bluetooth)",
          name: "耳机 (Bluetooth)",
          kind: "output",
          isDefault: false,
        },
      ] as T;

    case "audio_capture_status":
    case "stop_audio_capture":
      return {
        running: false,
        source: null,
        micRunning: false,
        systemRunning: false,
        micDevice: "麦克风 (USB Audio)",
        systemDevice: "扬声器 (Realtek)",
        loopbackAvailable: true,
      } as T;

    case "start_audio_capture": {
      const source = String(args?.source ?? "mic");
      return {
        running: true,
        source,
        micRunning: source === "mic",
        systemRunning: source === "system",
        micDevice: "麦克风 (USB Audio)",
        systemDevice: "扬声器 (Realtek)",
        loopbackAvailable: true,
      } as T;
    }

    // ---------- 本地 ASR（P0 唯一通路） ----------
    case "asr_session_status":
      return {
        running: false,
        sessionId: null,
        source: null,
        speaker: null,
        engine: null,
        startedAt: null,
        paused: false,
        dual: false,
        aecEnabled: false,
        aecErleDb: null,
      } as T;

    case "start_dual_asr_session":
      return {
        running: true,
        sessionId: "s-mock-dual",
        source: "dual",
        speaker: "interviewer",
        engine: "sherpa-onnx",
        startedAt: Date.now(),
        paused: false,
        dual: true,
        aecEnabled: true,
        aecErleDb: 0,
      } as T;

    case "set_asr_paused":
      return {
        running: true,
        sessionId: "s-mock-system",
        source: "system",
        speaker: "interviewer",
        engine: "sherpa-onnx",
        startedAt: Date.now(),
        paused: Boolean(args?.paused),
        dual: false,
        aecEnabled: false,
        aecErleDb: null,
      } as T;

    case "list_asr_engines":
      return [
        {
          id: "asr-paraformer-zh",
          label: "本地 ASR（sherpa-onnx 中文 paraformer）",
          ready: false,
          detail:
            "本地 ASR 模型尚未下载：C:\\Users\\demo\\AppData\\Roaming\\com.easyinterview.app\\models\\asr\\sherpa-onnx-paraformer-zh.tar.bz2。请前往「设置 → 模型管理」下载「本地 ASR（Paraformer 中文）」后重试。",
        },
      ] as T;

    case "start_asr_session": {
      const source = String(args?.source ?? "system");
      return {
        running: true,
        sessionId: `s-mock-${source}`,
        source,
        speaker: source === "system" ? "interviewer" : "me",
        engine: "sherpa-onnx",
        startedAt: Date.now(),
      } as T;
    }

    case "stop_asr_session":
      return {
        running: false,
        sessionId: null,
        source: null,
        speaker: null,
        engine: null,
        startedAt: null,
      } as T;

    case "list_transcript_segments":
    case "list_transcript_finals":
      return [] as T;

    // ---------- 问题识别（IA-06） ----------
    case "trigger_question":
      return null as T;

    case "list_questions":
      return [
        {
          id: "q-mock-1",
          sessionId: "s-mock-system",
          question: "介绍一下你最近负责的项目",
          answer: "",
          sources: [],
          ts: Date.now() - 30_000,
        },
      ] as T;

    case "latest_question":
      return null as T;

    case "get_question_config":
      return {
        autoDetect: true,
        threshold: 0.5,
        minChars: 4,
        mergeWindowMs: 3000,
      } as T;

    case "save_question_config":
      return (args?.config ?? null) as T;

    case "analyze_question_text":
      return {
        isQuestion: true,
        score: 0.95,
        signals: ["祈使式提问「介绍一下」"],
      } as T;

    case "set_stealth_window":
      return {
        theme: "dark",
        hotkeys: [],
        autoStart: false,
        trayBehavior: "minimize",
        alwaysOnTop: Boolean(args?.alwaysOnTop ?? false),
        hideFromCapture: Boolean(args?.hideFromCapture ?? false),
      } as T;

    case "apply_global_hotkeys":
      return [] as T;

    case "enter_mini_mode":
    case "exit_mini_mode":
      return null as T;

    case "start_demo_session":
      return {
        sessionId: "s-demo-1",
        notesSeeded: 4,
        questionTurns: 3,
        offlineAnswer: true,
        estimatedMs: 5400,
      } as T;

    case "generate_answer":
      return {
        qaId: "q-mock-1",
        question: "介绍一下你最近负责的项目",
        reused: false,
        sources: [
          {
            noteId: "note-mock-1",
            title: "项目架构设计",
            snippet: "负责桌面应用的架构设计与性能优化…",
            score: 0.0328,
          },
        ],
      } as T;

    case "get_question":
      return {
        id: String(args?.id ?? "q-mock-1"),
        sessionId: "s-mock-system",
        question: "介绍一下你最近负责的项目",
        answer: "我最近负责桌面端应用的架构设计，主要负责模块拆分与性能优化。",
        sources: [{ noteId: "note-mock-1", title: "项目架构设计" }],
        ts: Date.now(),
      } as T;

    // ---------- 更新检查（P2） ----------
    case "app_version":
      return "0.1.0" as T;

    case "get_update_config":
      return { autoCheck: false, endpoint: "" } as T;

    case "save_update_config":
      return (args?.config ?? { autoCheck: false, endpoint: "" }) as T;

    case "check_app_update":
      return {
        currentVersion: "0.1.0",
        latestVersion: "0.1.0",
        available: false,
        notes: null,
        url: null,
        message: "已是最新版本（0.1.0）",
      } as T;

    // ---------- 首次启动向导 ----------
    case "get_onboarding_state":
      // mock 环境默认视为已完成，便于直接浏览各页面
      return true as T;

    case "complete_onboarding":
      return true as T;

    // ---------- 会话记录（IA-10） ----------
    case "list_sessions":
      return [
        {
          id: "s-mock-1",
          title: "面试会话 · 双通道",
          startedAt: Date.now() - 3_600_000,
          endedAt: Date.now() - 2_400_000,
          segmentCount: 12,
          questionCount: 3,
        },
        {
          id: "s-mock-2",
          title: "面试会话 · system",
          startedAt: Date.now() - 86_400_000,
          endedAt: null,
          segmentCount: 4,
          questionCount: 1,
        },
      ] as T;

    case "get_session_detail": {
      const sessionId = String(args?.sessionId ?? "s-mock-1");
      const startedAt = Date.now() - 3_600_000;
      return {
        session: {
          id: sessionId,
          title: "面试会话 · 双通道",
          startedAt,
          endedAt: startedAt + 1_200_000,
        },
        segments: [
          {
            id: "t1",
            sessionId,
            speaker: "interviewer",
            text: "介绍一下你最近负责的项目",
            isFinal: true,
            ts: startedAt + 12_000,
          },
          {
            id: "t2",
            sessionId,
            speaker: "me",
            text: "我负责桌面端应用的架构设计",
            isFinal: true,
            ts: startedAt + 20_000,
          },
        ],
        qa: [
          {
            id: "q-mock-1",
            sessionId,
            question: "介绍一下你最近负责的项目",
            answer: "我负责桌面端应用架构设计，主要做模块拆分与性能优化。",
            sources: [{ noteId: "note-mock-1", title: "项目架构设计" }],
            ts: startedAt + 12_000,
          },
        ],
      } as T;
    }

    case "delete_session":
      return true as T;

    case "export_session":
      return {
        path: String(args?.dest ?? ""),
        format: String(args?.format ?? "markdown"),
        segments: 2,
        questions: 1,
        bytes: 2048,
      } as T;

    case "recall_for_question":
      return {
        question: "介绍一下你最近负责的项目",
        queries: ["介绍一下你最近负责的项目", "最近 负责 项目"],
        hits: [
          {
            noteId: "note-mock-1",
            title: "项目架构设计",
            snippet: "负责桌面应用的架构设计与性能优化…",
            score: 0.0328,
          },
        ],
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
          provider: "aliyun",
          label: "阿里云智能语音",
          // ST-14：与后端 `CloudAsrProvider::Aliyun.default_model()` 保持一致
          defaultModel: "qwen-audio-3.0-asr-flash-streaming",
          credentialFields: ["api_key"],
          defaultBaseUrl: "wss://dashscope.aliyuncs.com/api-ws/v1/inference",
          implemented: true,
          usesWebsocket: true,
        },
        {
          provider: "xfyun",
          label: "讯飞开放平台",
          defaultModel: "iat",
          credentialFields: ["app_id", "api_key", "api_secret"],
          defaultBaseUrl: "wss://ws-api.xfyun.cn/v2/iat",
          implemented: false,
          usesWebsocket: true,
        },
        {
          provider: "tencent",
          label: "腾讯云语音识别",
          defaultModel: "16k_zh",
          credentialFields: ["secret_id", "secret_key"],
          defaultBaseUrl: "wss://asr.cloud.tencent.com/asr/v2",
          implemented: false,
          usesWebsocket: true,
        },
        {
          provider: "openai_compatible",
          label: "OpenAI 兼容（自定义）",
          defaultModel: "whisper-1",
          credentialFields: ["api_key"],
          defaultBaseUrl: "https://api.openai.com/v1",
          implemented: false,
          usesWebsocket: false,
        },
      ] as T;

    case "get_asr_config":
      // ST-14：与后端 `AsrConfig::default()`（云端 + 阿里云 + 3.0 流式模型）保持一致
      return {
        mode: "cloud",
        cloudProvider: "aliyun",
        model: "qwen-audio-3.0-asr-flash-streaming",
        language: "zh-CN",
        vadThreshold: 0.5,
        localModel: "sherpa-onnx-paraformer-zh",
        cloudBaseUrls: {},
      } as T;

    case "save_asr_config":
      return (args?.config ?? null) as T;

    case "test_cloud_asr_connection":
      return {
        ok: true,
        latencyMs: 128,
        message: "阿里云智能语音 连接成功（128ms）",
      } as T;

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
