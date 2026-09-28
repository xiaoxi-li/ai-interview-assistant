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

interface SettingsState {
  /** 操作提示（成功类信息） */
  notice: string | null;
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

  const fail = (action: string, err: unknown) => {
    console.error(`${action}失败:`, err);
    set({ error: String(err) });
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
        .then((saved) => set({ interfaceConfig: saved }))
        .catch((err) => fail("保存界面配置", err)),

    setAutoStart: (enabled) =>
      ipc<boolean>("set_autostart", { enabled })
        .then((autoStartEnabled) => set({ autoStartEnabled }))
        .catch((err) => fail("设置开机自启", err)),

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
        .then((saved) => set({ knowledgeConfig: saved }))
        .catch((err) => fail("保存知识库配置", err)),
    privacyConfig: null,
    clearing: false,
    notice: null,
    error: null,

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
        .then((saved) => set({ privacyConfig: saved }))
        .catch((err) => fail("保存隐私配置", err)),

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
      ipc<AudioDevice[]>("list_audio_devices")
        .then((audioDevices) => set({ audioDevices }))
        .catch((err) => fail("刷新音频设备", err)),

    saveAudioConfig: (config) =>
      ipc<AudioConfig>("save_audio_config", { config })
        .then((saved) => set({ audioConfig: saved }))
        .catch((err) => fail("保存音频设备配置", err)),

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
        .then((saved) => set({ asrConfig: saved }))
        .catch((err) => fail("保存 ASR 配置", err)),

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
        .then((status) =>
          set((state) => ({
            asrCredentials: { ...state.asrCredentials, [status.provider]: status },
          }))
        )
        .catch((err) => fail("保存 ASR 凭证", err)),

    deleteAsrCredentials: (provider) =>
      ipc<AsrCredentialStatus>("delete_asr_credentials", { provider })
        .then((status) =>
          set((state) => ({
            asrCredentials: { ...state.asrCredentials, [status.provider]: status },
          }))
        )
        .catch((err) => fail("清除 ASR 凭证", err)),

    loadAiSettings: () =>
      Promise.all([
        ipc<AIConfig>("get_ai_config"),
        ipc<PlatformPreset[]>("list_platform_presets"),
      ])
        .then(([aiConfig, presets]) => set({ aiConfig, presets }))
        .catch((err) => fail("加载 AI 配置", err)),

    saveAiConfig: (config) =>
      ipc<AIConfig>("save_ai_config", { config })
        .then((saved) => set({ aiConfig: saved }))
        .catch((err) => fail("保存 AI 配置", err)),

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
        .then(apply)
        .catch((err) => fail("保存密钥", err)),

    deleteApiKey: (platform) =>
      ipc<ApiKeyStatus>("delete_api_key", { platform })
        .then(apply)
        .catch((err) => fail("删除密钥", err)),
  };
});
