import type { AIConfig, PlatformPreset } from "../types";

/** temperature 合法区间（与后端 AiConfig::validate 保持一致） */
export const TEMPERATURE_MIN = 0;
export const TEMPERATURE_MAX = 2;

/**
 * 前端侧配置校验：与后端 `AiConfig::validate` 规则一致，
 * 用于即时反馈，避免无谓的 IPC 往返。
 */
export function validateAiConfig(config: AIConfig): string | null {
  if (!config.baseUrl.trim()) {
    return "Base URL 不能为空";
  }
  if (!/^https?:\/\//.test(config.baseUrl.trim())) {
    return "Base URL 需以 http:// 或 https:// 开头";
  }
  if (!config.model.trim()) {
    return "模型名不能为空";
  }
  if (config.temperature < TEMPERATURE_MIN || config.temperature > TEMPERATURE_MAX) {
    return `温度需在 ${TEMPERATURE_MIN} ~ ${TEMPERATURE_MAX} 之间`;
  }
  return null;
}

/** 判断配置是否与持久化值一致（用于"未保存"提示） */
export function isAiConfigDirty(draft: AIConfig, saved: AIConfig | null): boolean {
  if (!saved) return true;
  return (
    draft.platform !== saved.platform ||
    draft.baseUrl !== saved.baseUrl ||
    draft.model !== saved.model ||
    draft.stream !== saved.stream ||
    draft.temperature !== saved.temperature
  );
}

/** 平台显示名（找不到预设时回退到平台标识） */
export function platformLabel(
  platform: AIConfig["platform"],
  presets: PlatformPreset[]
): string {
  return presets.find((p) => p.platform === platform)?.label ?? platform;
}
