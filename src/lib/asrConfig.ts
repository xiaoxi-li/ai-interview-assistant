import type { ASRConfig } from "../types";

/** VAD 阈值合法区间（与后端一致） */
export const VAD_MIN = 0;
export const VAD_MAX = 1;

/** 凭证字段显示名 */
const FIELD_LABELS: Record<string, string> = {
  app_id: "AppID",
  api_key: "API Key",
  api_secret: "API Secret",
  secret_id: "SecretId",
  secret_key: "SecretKey",
};

export function credentialFieldLabel(field: string): string {
  return FIELD_LABELS[field] ?? field;
}

/**
 * 前端侧 ASR 配置校验：与后端 `AsrConfig::validate` 规则一致。
 */
export function validateAsrConfig(config: ASRConfig): string | null {
  if (!config.model.trim()) {
    return "云端模型名不能为空";
  }
  if (!config.localModel.trim()) {
    return "本地模型名不能为空";
  }
  if (!config.language.trim()) {
    return "识别语言不能为空";
  }
  if (config.vadThreshold < VAD_MIN || config.vadThreshold > VAD_MAX) {
    return `VAD 阈值需在 ${VAD_MIN} ~ ${VAD_MAX} 之间`;
  }
  return null;
}

/** 当前模式实际生效的模型名 */
export function activeAsrModel(config: ASRConfig): string {
  return config.mode === "local" ? config.localModel : config.model;
}

/** 云端模式是否缺少必要凭证 */
export function missingCredentials(
  config: ASRConfig,
  configuredFields: string[]
): string[] {
  if (config.mode !== "cloud") return [];
  const preset = ASR_PROVIDER_FIELDS[config.cloudProvider] ?? [];
  return preset.filter((field) => !configuredFields.includes(field));
}

/** 各服务商所需凭证字段（与后端 credential_fields 对应） */
export const ASR_PROVIDER_FIELDS: Record<string, string[]> = {
  xfyun: ["app_id", "api_key", "api_secret"],
  aliyun: ["api_key"],
  tencent: ["secret_id", "secret_key"],
};
