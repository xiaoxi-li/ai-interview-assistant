import { create } from "zustand";
import { ipc } from "../lib/ipc";

/** API Key 配置状态（后端只回传掩码，不泄露完整密钥） */
export interface ApiKeyStatus {
  platform: string;
  configured: boolean;
  masked: string;
}

interface SettingsState {
  /** 各平台的密钥配置状态 */
  apiKeys: Record<string, ApiKeyStatus>;
  error: string | null;
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
    error: null,

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
