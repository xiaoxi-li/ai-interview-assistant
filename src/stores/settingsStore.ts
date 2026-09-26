import { create } from "zustand";
import { ipc } from "../lib/ipc";
import type {
  AIConfig,
  ASRConfig,
  AsrCredentialStatus,
  AsrProviderPreset,
  ConnectionTest,
  PlatformPreset,
} from "../types";

/** API Key 配置状态（后端只回传掩码，不泄露完整密钥） */
export interface ApiKeyStatus {
  platform: string;
  configured: boolean;
  masked: string;
}

interface SettingsState {
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
  loadAsrSettings: () => Promise<void>;
  saveAsrConfig: (config: ASRConfig) => Promise<void>;
  loadAsrCredentialStatus: (provider: string) => Promise<void>;
  saveAsrCredential: (provider: string, field: string, value: string) => Promise<void>;
  deleteAsrCredentials: (provider: string) => Promise<void>;
  /** 查询指定平台的密钥状态 */
  loadApiKeyStatus: (platform: string) => Promise<void>;
  /** 保存密钥（空串表示清除） */
  saveApiKey: (platform: string, apiKey: string) => Promise<void>;
  /** 删除密钥 */
  deleteApiKey: (platform: string) => Promise<void>;
}

export const useSettingsStore = create<SettingsState>((set) => {
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
    error: null,

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
