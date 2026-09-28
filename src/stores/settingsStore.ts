import { create } from "zustand";
import { ipc } from "../lib/ipc";
import type {
  AIConfig,
  ASRConfig,
  AsrCredentialStatus,
  AsrProviderPreset,
  AudioConfig,
  AudioDevice,
  ConnectionTest,
  InterfaceConfig,
  InterfaceTheme,
  KnowledgeConfig,
  ModelProgress,
  ModelStatus,
  PlatformPreset,
  PrivacyConfig,
  StoragePaths,
} from "../types";

/** API Key 配置状态（后端只回传掩码，不泄露完整密钥） */
export interface ApiKeyStatus {
  platform: string;
  configured: boolean;
  masked: string;
}

/** 设置分区标识：保存反馈按分区就地展示（ST-11） */
export type SettingsFeedbackTarget =
  | "ai"
  | "asr"
  | "audio"
  | "knowledge"
  | "interface"
  | "privacy";

/** 分区保存反馈：成功约 3 秒自动淡出，失败保留至手动关闭或下一次操作 */
export interface SettingsSaveFeedback {
  kind: "success" | "error";
  text: string;
}

/** 空的分区反馈表（state 初始化与测试重置共用） */
export const emptySaveFeedback = (): Record<
  SettingsFeedbackTarget,
  SettingsSaveFeedback | null
> => ({
  ai: null,
  asr: null,
  audio: null,
  knowledge: null,
  interface: null,
  privacy: null,
});

interface SettingsState {
  /** 操作提示（成功类信息） */
  notice: string | null;
  /** 各设置分区的保存结果反馈（ST-11：就地提示成功/失败） */
  saveFeedback: Record<SettingsFeedbackTarget, SettingsSaveFeedback | null>;
  /** 写入某个分区的保存反馈 */
  setSaveFeedback: (
    target: SettingsFeedbackTarget,
    feedback: SettingsSaveFeedback
  ) => void;
  /** 清除某个分区的保存反馈（成功提示到期 / 用户关闭失败提示） */
  clearSaveFeedback: (target: SettingsFeedbackTarget) => void;
  /** 各平台的密钥配置状态 */
  apiKeys: Record<string, ApiKeyStatus>;
  /** 当前 AI 配置 */
  aiConfig: AIConfig | null;
  /** 平台预设列表 */
  presets: PlatformPreset[];
  error: string | null;
  /** 加载 AI 配置与平台预设 */
  loadAiSettings: () => Promise<void>;
  /** 保存 AI 配置 */
  saveAiConfig: (config: AIConfig) => Promise<void>;
  /** 连接测试进行中 */
  testing: boolean;
  /** 最近一次连接测试结果 */
  testResult: ConnectionTest | null;
  /** 一键测试 AI 服务连通性 */
  testAiConnection: () => Promise<void>;
  /** ASR 配置 */
  asrConfig: ASRConfig | null;
  /** 云端 ASR 服务商预设 */
  asrProviders: AsrProviderPreset[];
  /** 各服务商凭证状态（按 provider 键） */
  asrCredentials: Record<string, AsrCredentialStatus>;
  /** 云端 ASR 连接测试进行中 */
  testingAsr: boolean;
  /** 最近一次云端 ASR 连接测试结果 */
  asrTestResult: ConnectionTest | null;
  /** 一键测试云端 ASR 连通性（IA-16.8）；传入表单草稿则测草稿，缺省测已保存配置 */
  testAsrConnection: (config?: ASRConfig) => Promise<void>;
  loadAsrSettings: () => Promise<void>;
  saveAsrConfig: (config: ASRConfig) => Promise<void>;
  loadAsrCredentialStatus: (provider: string) => Promise<void>;
  saveAsrCredential: (provider: string, field: string, value: string) => Promise<void>;
  deleteAsrCredentials: (provider: string) => Promise<void>;
  /** 音频设备配置 */
  audioConfig: AudioConfig | null;
  /** 已枚举的设备列表 */
  audioDevices: AudioDevice[];
  loadAudioSettings: () => Promise<void>;
  refreshAudioDevices: () => Promise<void>;
  saveAudioConfig: (config: AudioConfig) => Promise<void>;
  /** 界面配置 */
  interfaceConfig: InterfaceConfig | null;
  /** 系统级开机自启状态 */
  autoStartEnabled: boolean;
  /**
   * 设置页正在预览的主题（ST-13）：非空表示"即时预览中、尚未保存"。
   * 取值优先于 `interfaceConfig.theme`，由 `useThemeSync` 统一消费。
   */
  previewTheme: InterfaceTheme | null;
  /** 设置页点选主题：仅即时预览，不持久化（保存由 `saveInterfaceConfig` 完成） */
  setPreviewTheme: (theme: InterfaceTheme) => void;
  /** 结束预览并回滚到已保存主题（离开设置页时调用） */
  clearPreviewTheme: () => void;
  loadInterfaceSettings: () => Promise<void>;
  saveInterfaceConfig: (config: InterfaceConfig) => Promise<void>;
  setAutoStart: (enabled: boolean) => Promise<void>;
  /** 切换隐身模式开关（窗口置顶 / 防录屏，IA-11） */
  setStealthWindow: (patch: {
    alwaysOnTop?: boolean;
    hideFromCapture?: boolean;
  }) => Promise<void>;
  /** 知识库配置 */
  knowledgeConfig: KnowledgeConfig | null;
  /** 当前生效的存储路径 */
  storagePaths: StoragePaths | null;
  loadKnowledgeSettings: () => Promise<void>;
  saveKnowledgeConfig: (config: KnowledgeConfig) => Promise<void>;
  /** 模型清单与安装状态 */
  models: ModelStatus[];
  /** 下载进度（按模型 ID） */
  modelProgress: Record<string, ModelProgress>;
  /** 正在下载的模型 ID */
  downloadingId: string | null;
  /** 各模型最近一次下载失败信息（内联展示 + 重试） */
  modelErrors: Record<string, string>;
  loadModels: () => Promise<void>;
  downloadModel: (id: string) => Promise<void>;
  deleteModel: (id: string) => Promise<void>;
  applyModelProgress: (progress: ModelProgress) => void;
  clearModelError: (id: string) => void;
  /** 隐私配置 */
  privacyConfig: PrivacyConfig | null;
  /** 清除本地数据中 */
  clearing: boolean;
  loadPrivacySettings: () => Promise<void>;
  savePrivacyConfig: (config: PrivacyConfig) => Promise<void>;
  clearLocalData: () => Promise<void>;
  /** 查询指定平台的密钥状态 */
  loadApiKeyStatus: (platform: string) => Promise<void>;
  /** 保存密钥（空串表示清除） */
  saveApiKey: (platform: string, apiKey: string) => Promise<void>;
  /** 删除密钥 */
  deleteApiKey: (platform: string) => Promise<void>;
}

export const useSettingsStore = create<SettingsState>((set, get) => {
  const apply = (status: ApiKeyStatus) =>
    set((state) => ({
      apiKeys: { ...state.apiKeys, [status.platform]: status },
    }));

  /** 写某个分区的保存反馈；success 由 UI 侧定时自动清除，error 由用户关闭或下次操作覆盖 */
  const feedback = (
    target: SettingsFeedbackTarget,
    kind: SettingsSaveFeedback["kind"],
    text: string
  ) =>
    set((state) => ({
      saveFeedback: { ...state.saveFeedback, [target]: { kind, text } },
    }));

  const fail = (action: string, err: unknown, target?: SettingsFeedbackTarget) => {
    console.error(`${action}失败:`, err);
    // Error 只取 message，避免提示里出现多余的「Error: 」前缀
    const message = err instanceof Error ? err.message : String(err);
    set((state) => ({
      error: message,
      saveFeedback: target
        ? {
            ...state.saveFeedback,
            [target]: { kind: "error", text: `${action}失败：${message}` },
          }
        : state.saveFeedback,
    }));
  };

  return {
    apiKeys: {},
    aiConfig: null,
    presets: [],
    testing: false,
    testResult: null,
    asrConfig: null,
    asrProviders: [],
    asrCredentials: {},
    testingAsr: false,
    asrTestResult: null,
    audioConfig: null,
    audioDevices: [],
    interfaceConfig: null,
    autoStartEnabled: false,
    previewTheme: null,

    setPreviewTheme: (theme) => set({ previewTheme: theme }),

    clearPreviewTheme: () => set({ previewTheme: null }),

    knowledgeConfig: null,
    storagePaths: null,

    loadInterfaceSettings: () =>
      Promise.all([
        ipc<InterfaceConfig>("get_interface_config"),
        ipc<boolean>("get_autostart"),
      ])
        .then(([interfaceConfig, autoStartEnabled]) =>
          set({ interfaceConfig, autoStartEnabled })
        )
        .catch((err) => fail("加载界面配置", err)),

    saveInterfaceConfig: (config) =>
      ipc<InterfaceConfig>("save_interface_config", { config })
        .then((saved) => {
          // 保存成功后预览态即成为正式主题，清空以免与已保存值分叉（ST-13）
          set({ interfaceConfig: saved, previewTheme: null });
          feedback("interface", "success", "界面配置已保存");
        })
        .catch((err) => fail("保存界面配置", err, "interface")),

    setAutoStart: (enabled) =>
      ipc<boolean>("set_autostart", { enabled })
        .then((autoStartEnabled) => {
          set({ autoStartEnabled });
          feedback(
            "interface",
            "success",
            autoStartEnabled ? "开机自启已开启" : "开机自启已关闭"
          );
        })
        .catch((err) => fail("设置开机自启", err, "interface")),

    setStealthWindow: (patch) =>
      ipc<InterfaceConfig>("set_stealth_window", patch)
        .then((interfaceConfig) => set({ interfaceConfig }))
        .catch((err) => fail("切换隐身模式", err)),
    models: [],
    modelProgress: {},
    downloadingId: null,
    modelErrors: {},

    loadKnowledgeSettings: () =>
      Promise.all([
        ipc<KnowledgeConfig>("get_knowledge_config"),
        ipc<StoragePaths>("get_storage_paths"),
      ])
        .then(([knowledgeConfig, storagePaths]) =>
          set({ knowledgeConfig, storagePaths })
        )
        .catch((err) => fail("加载知识库配置", err)),

    saveKnowledgeConfig: (config) =>
      ipc<KnowledgeConfig>("save_knowledge_config", { config })
        .then((saved) => {
          set({ knowledgeConfig: saved });
          feedback("knowledge", "success", "知识库配置已保存");
        })
        .catch((err) => fail("保存知识库配置", err, "knowledge")),
    privacyConfig: null,
    clearing: false,
    notice: null,
    error: null,
    saveFeedback: emptySaveFeedback(),

    setSaveFeedback: (target, value) => feedback(target, value.kind, value.text),

    clearSaveFeedback: (target) =>
      set((state) => ({
        saveFeedback: { ...state.saveFeedback, [target]: null },
      })),

    loadModels: () =>
      ipc<ModelStatus[]>("list_models")
        .then((models) => set({ models }))
        .catch((err) => fail("加载模型清单", err)),

    applyModelProgress: (progress) =>
      set((state) => ({
        modelProgress: { ...state.modelProgress, [progress.id]: progress },
      })),

    downloadModel: (id) => {
      set((state) => ({
        downloadingId: id,
        error: null,
        // 清空该模型旧错误，重置进度（断点续传会由后端重新上报 offset）
        modelErrors: { ...state.modelErrors, [id]: "" },
        modelProgress: {
          ...state.modelProgress,
          [id]: { id, downloaded: 0, total: null },
        },
      }));
      return ipc<string>("download_model", { id })
        .then(() => {
          set((state) => {
            const modelErrors = { ...state.modelErrors };
            delete modelErrors[id];
            return {
              downloadingId: null,
              modelErrors,
              notice: "模型下载完成",
            };
          });
          return get().loadModels();
        })
        .catch((err) => {
          // 失败保留进度与缓存，内联展示错误供重试，而非「闪烁消失」
          const message = String(err);
          console.error("下载模型失败:", err);
          set((state) => ({
            downloadingId: null,
            modelErrors: { ...state.modelErrors, [id]: message },
          }));
        });
    },

    clearModelError: (id) =>
      set((state) => {
        const modelErrors = { ...state.modelErrors };
        delete modelErrors[id];
        return { modelErrors };
      }),

    deleteModel: (id) =>
      ipc<boolean>("delete_model", { id })
        .then(() => {
          set((state) => {
            const next = { ...state.modelProgress };
            delete next[id];
            return { modelProgress: next, notice: "模型已删除" };
          });
          return get().loadModels();
        })
        .catch((err) => fail("删除模型", err)),

    loadPrivacySettings: () =>
      ipc<PrivacyConfig>("get_privacy_config")
        .then((privacyConfig) => set({ privacyConfig }))
        .catch((err) => fail("加载隐私配置", err)),

    savePrivacyConfig: (config) =>
      ipc<PrivacyConfig>("save_privacy_config", { config })
        .then((saved) => {
          set({ privacyConfig: saved });
          feedback("privacy", "success", "隐私配置已保存");
        })
        .catch((err) => fail("保存隐私配置", err, "privacy")),

    clearLocalData: () => {
      set({ clearing: true, error: null });
      return ipc<{ notes: number; sessions: number }>("clear_local_data")
        .then((summary) =>
          set({
            clearing: false,
            notice: `已清除本地数据（${summary.notes} 篇笔记、${summary.sessions} 场会话）`,
          })
        )
        .catch((err) => {
          set({ clearing: false });
          fail("清除本地数据", err);
        });
    },

    loadAudioSettings: () =>
      Promise.all([
        ipc<AudioConfig>("get_audio_config"),
        ipc<AudioDevice[]>("list_audio_devices"),
      ])
        .then(([audioConfig, audioDevices]) => set({ audioConfig, audioDevices }))
        .catch((err) => fail("加载音频设备配置", err)),

    refreshAudioDevices: () =>
      // force = true：旁路后端设备枚举的 TTL 缓存，保证「刷新」立即反映设备插拔
      ipc<AudioDevice[]>("list_audio_devices", { force: true })
        .then((audioDevices) => {
          set({ audioDevices });
          feedback("audio", "success", "设备列表已刷新");
        })
        .catch((err) => fail("刷新音频设备", err, "audio")),

    saveAudioConfig: (config) =>
      ipc<AudioConfig>("save_audio_config", { config })
        .then((saved) => {
          set({ audioConfig: saved });
          feedback("audio", "success", "音频配置已保存");
        })
        .catch((err) => fail("保存音频设备配置", err, "audio")),

    loadAsrSettings: () =>
      Promise.all([
        ipc<ASRConfig>("get_asr_config"),
        ipc<AsrProviderPreset[]>("list_asr_providers"),
      ])
        .then(([asrConfig, asrProviders]) => {
          set({ asrConfig, asrProviders });
          return ipc<AsrCredentialStatus>("get_asr_credential_status", {
            provider: asrConfig.cloudProvider,
          })
            .then((status) =>
              set((state) => ({
                asrCredentials: {
                  ...state.asrCredentials,
                  [status.provider]: status,
                },
              }))
            )
            .catch((err) => fail("查询 ASR 凭证状态", err));
        })
        .catch((err) => fail("加载 ASR 配置", err)),

    saveAsrConfig: (config) =>
      ipc<ASRConfig>("save_asr_config", { config })
        .then((saved) => {
          set({ asrConfig: saved });
          feedback("asr", "success", "ASR 配置已保存");
        })
        .catch((err) => fail("保存 ASR 配置", err, "asr")),

    testAsrConnection: (config) => {
      set({ testingAsr: true, asrTestResult: null, error: null });
      return ipc<ConnectionTest>("test_cloud_asr_connection", { config })
        .then((result) => set({ testingAsr: false, asrTestResult: result }))
        .catch((err) => {
          console.error("云端 ASR 连接测试失败:", err);
          set({
            testingAsr: false,
            asrTestResult: { ok: false, latencyMs: 0, message: String(err) },
          });
        });
    },

    loadAsrCredentialStatus: (provider) =>
      ipc<AsrCredentialStatus>("get_asr_credential_status", { provider })
        .then((status) =>
          set((state) => ({
            asrCredentials: { ...state.asrCredentials, [status.provider]: status },
          }))
        )
        .catch((err) => fail("查询 ASR 凭证状态", err)),

    saveAsrCredential: (provider, field, value) =>
      ipc<AsrCredentialStatus>("save_asr_credential", { provider, field, value })
        .then((status) => {
          set((state) => ({
            asrCredentials: { ...state.asrCredentials, [status.provider]: status },
          }));
          feedback("asr", "success", "凭证已保存");
        })
        .catch((err) => fail("保存 ASR 凭证", err, "asr")),

    deleteAsrCredentials: (provider) =>
      ipc<AsrCredentialStatus>("delete_asr_credentials", { provider })
        .then((status) => {
          set((state) => ({
            asrCredentials: { ...state.asrCredentials, [status.provider]: status },
          }));
          feedback("asr", "success", "凭证已清空");
        })
        .catch((err) => fail("清除 ASR 凭证", err, "asr")),

    loadAiSettings: () =>
      Promise.all([
        ipc<AIConfig>("get_ai_config"),
        ipc<PlatformPreset[]>("list_platform_presets"),
      ])
        .then(([aiConfig, presets]) => set({ aiConfig, presets }))
        .catch((err) => fail("加载 AI 配置", err)),

    saveAiConfig: (config) =>
      ipc<AIConfig>("save_ai_config", { config })
        .then((saved) => {
          set({ aiConfig: saved });
          feedback("ai", "success", "AI 配置已保存");
        })
        .catch((err) => fail("保存 AI 配置", err, "ai")),

    testAiConnection: () => {
      set({ testing: true, testResult: null, error: null });
      return ipc<ConnectionTest>("test_ai_connection")
        .then((result) => set({ testing: false, testResult: result }))
        .catch((err) => {
          console.error("连接测试失败:", err);
          set({
            testing: false,
            testResult: { ok: false, latencyMs: 0, message: String(err) },
          });
        });
    },

    loadApiKeyStatus: (platform) =>
      ipc<ApiKeyStatus>("get_api_key_status", { platform })
        .then(apply)
        .catch((err) => fail("查询密钥状态", err)),

    saveApiKey: (platform, apiKey) =>
      ipc<ApiKeyStatus>("save_api_key", { platform, apiKey })
        .then((status) => {
          apply(status);
          feedback("ai", "success", "密钥已保存");
        })
        .catch((err) => fail("保存密钥", err, "ai")),

    deleteApiKey: (platform) =>
      ipc<ApiKeyStatus>("delete_api_key", { platform })
        .then((status) => {
          apply(status);
          feedback("ai", "success", "密钥已清除");
        })
        .catch((err) => fail("删除密钥", err, "ai")),
  };
});
