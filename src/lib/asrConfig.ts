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

/**
 * 仅提供 HTTP REST 的常见路径形态（OpenAI 兼容 / 原生接口 / Anthropic 兼容）。
 * WebSocket 实时识别无法走这些路径（握手会返回 404），故给出提示。
 */
const HTTP_ONLY_PATH_PATTERNS = ["compatible-mode", "/api/v1", "/apps/anthropic"];

/**
 * 服务端地址「疑似填成 HTTP 接口」的提示（仅 WebSocket 服务商，非阻塞保存）。
 *
 * 典型误填：把 OpenAI 兼容的 `https://dashscope.aliyuncs.com/compatible-mode/v1`
 * 填给实时识别——它是 HTTP 接口，WebSocket 握手会得到 404。
 */
export function baseUrlWarning(
  value: string | undefined | null,
  usesWebsocket: boolean
): string | null {
  const trimmed = (value ?? "").trim();
  if (!trimmed || !usesWebsocket) return null;

  const lower = trimmed.toLowerCase();
  if (!HTTP_ONLY_PATH_PATTERNS.some((pattern) => lower.includes(pattern))) {
    return null;
  }
  return "该路径属于 HTTP 接口（OpenAI 兼容 / 原生 REST），实时转写不支持，握手会返回 404；请改用 WebSocket 地址（例如 wss://dashscope.aliyuncs.com/api-ws/v1/inference），或点「恢复默认」";
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
