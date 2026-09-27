import type { ASRConfig, CloudAsrProvider } from "../types";

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
  for (const [provider, value] of Object.entries(config.cloudBaseUrls ?? {})) {
    const error = validateBaseUrl(value);
    if (error) {
      const label = ASR_PROVIDER_LABELS[provider as CloudAsrProvider] ?? provider;
      return `${label}：${error}`;
    }
  }
  return null;
}

/** Base URL 允许的协议前缀（与后端 `BASE_URL_SCHEMES` 一致） */
const BASE_URL_SCHEMES = ["ws://", "wss://", "http://", "https://"];

/** 校验单个服务商的自定义 Base URL（仅在填写了值时调用） */
export function validateBaseUrl(value: string | undefined | null): string | null {
  const trimmed = (value ?? "").trim();
  if (!trimmed) return null;
  if (!BASE_URL_SCHEMES.some((scheme) => trimmed.startsWith(scheme))) {
    return "服务端地址需以 ws://、wss://、http:// 或 https:// 开头";
  }
  return null;
}

/** 某服务商生效的服务端地址：自定义值优先，否则回退官方默认 */
export function effectiveBaseUrl(
  config: ASRConfig,
  provider: CloudAsrProvider,
  defaultBaseUrl: string
): string {
  const custom = config.cloudBaseUrls?.[provider]?.trim();
  return custom ? custom : defaultBaseUrl;
}

/** 某服务商是否使用了自定义地址 */
export function hasCustomBaseUrl(
  config: ASRConfig,
  provider: CloudAsrProvider
): boolean {
  return Boolean(config.cloudBaseUrls?.[provider]?.trim());
}

/**
 * 写入某服务商的自定义地址（**每个服务商各存一份**，互不覆盖）。
 * 空白值表示恢复官方默认（移除该项，保持与后端 `set_base_url` 语义一致）。
 */
export function withProviderBaseUrl(
  config: ASRConfig,
  provider: CloudAsrProvider,
  value: string
): ASRConfig {
  const next = { ...(config.cloudBaseUrls ?? {}) };
  const trimmed = value.trim();
  if (trimmed) {
    next[provider] = trimmed;
  } else {
    delete next[provider];
  }
  return { ...config, cloudBaseUrls: next };
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
  openai_compatible: ["api_key"],
};

/** 服务商显示名（后端未提供预设时的兜底文案） */
export const ASR_PROVIDER_LABELS: Record<string, string> = {
  xfyun: "讯飞开放平台",
  aliyun: "阿里云智能语音",
  tencent: "腾讯云语音识别",
  openai_compatible: "OpenAI 兼容（自定义）",
};
