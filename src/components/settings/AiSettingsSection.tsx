import { useEffect, useMemo, useState } from "react";
import {
  AlertCircle,
  Check,
  KeyRound,
  Loader2,
  Plug,
  Save,
  Trash2,
} from "lucide-react";
import { cn } from "../../lib/utils";
import { SaveFeedback } from "./SaveFeedback";
import {
  isAiConfigDirty,
  thinkingParamHint,
  validateAiConfig,
} from "../../lib/aiConfig";
import type {
  AIConfig,
  ConnectionTest,
  PlatformPreset,
  ThinkingMode,
} from "../../types";
import type { ApiKeyStatus } from "../../stores/settingsStore";

const FIELD =
  "focus-ring w-full rounded-lg border border-ghost/10 bg-background-3/60 px-3 py-2 text-[13px] text-text outline-none transition-colors placeholder:text-text-subtle focus:border-primary/50";
const LABEL = "mb-1.5 block text-[11px] text-text-muted";

/** 回答模式选项（ST-12）：默认非思考 */
const THINKING_OPTIONS: { value: ThinkingMode; label: string }[] = [
  { value: "nonThinking", label: "非思考" },
  { value: "thinking", label: "思考" },
];

interface AiSettingsSectionProps {
  config: AIConfig;
  presets: PlatformPreset[];
  apiKeyStatus?: ApiKeyStatus;
  onSaveConfig: (config: AIConfig) => void;
  onSaveApiKey: (platform: string, apiKey: string) => void;
  onDeleteApiKey: (platform: string) => void;
  /** 一键连接测试 */
  onTestConnection?: () => void;
  testing?: boolean;
  testResult?: ConnectionTest | null;
}

export default function AiSettingsSection({
  config,
  presets,
  apiKeyStatus,
  onSaveConfig,
  onSaveApiKey,
  onDeleteApiKey,
  onTestConnection,
  testing = false,
  testResult = null,
}: AiSettingsSectionProps) {
  const [draft, setDraft] = useState<AIConfig>(config);
  const [apiKey, setApiKey] = useState("");

  const validationError = useMemo(() => validateAiConfig(draft), [draft]);
  const dirty = isAiConfigDirty(draft, config);
  const activePreset = presets.find((preset) => preset.platform === draft.platform);

  // 外部配置变化（如首次加载）时同步草稿
  useEffect(() => {
    setDraft(config);
  }, [config]);

  const applyPreset = (platform: AIConfig["platform"]) => {
    const preset = presets.find((p) => p.platform === platform);
    setDraft((prev) => ({
      ...prev,
      platform,
      baseUrl: preset?.defaultBaseUrl ?? prev.baseUrl,
      model: preset?.defaultModel ?? prev.model,
    }));
  };

  const saveKey = () => {
    const value = apiKey.trim();
    if (!value) return;
    onSaveApiKey(draft.platform, value);
    setApiKey("");
  };

  return (
    <section className="glass rounded-xl p-4 shadow-card">
      <header className="mb-4 flex items-center gap-2">
        <h2 className="text-[13px] font-semibold">AI 平台</h2>
        <span className="text-[11px] text-text-subtle">
          用于生成面试回答的对话模型
        </span>
      </header>

      {/* 平台选择 */}
      <div className="mb-4 flex flex-wrap gap-1.5">
        {presets.map((preset) => (
          <button
            key={preset.platform}
            onClick={() => applyPreset(preset.platform)}
            className={cn(
              "focus-ring cursor-pointer rounded-lg border px-3 py-1.5 text-[12px] transition-colors",
              draft.platform === preset.platform
                ? "border-primary/40 bg-primary/15 text-primary-3"
                : "border-ghost/10 bg-background-3/60 text-text-muted hover:text-text"
            )}
          >
            {preset.label}
          </button>
        ))}
      </div>

      <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
        <div>
          <label className={LABEL} htmlFor="ai-base-url">
            Base URL
          </label>
          <input
            id="ai-base-url"
            value={draft.baseUrl}
            onChange={(e) => setDraft({ ...draft, baseUrl: e.target.value })}
            placeholder="https://dashscope.aliyuncs.com/compatible-mode/v1"
            className={FIELD}
          />
        </div>

        <div>
          <label className={LABEL} htmlFor="ai-model">
            模型名
          </label>
          <input
            id="ai-model"
            value={draft.model}
            onChange={(e) => setDraft({ ...draft, model: e.target.value })}
            placeholder="qwen3.8-flash"
            className={FIELD}
          />
        </div>

        <div>
          <label className={LABEL} htmlFor="ai-temperature">
            温度（0 ~ 2）：{draft.temperature.toFixed(1)}
          </label>
          <input
            id="ai-temperature"
            type="range"
            min={0}
            max={2}
            step={0.1}
            value={draft.temperature}
            onChange={(e) =>
              setDraft({ ...draft, temperature: Number(e.target.value) })
            }
            className="w-full cursor-pointer accent-primary"
          />
        </div>

        <div className="flex items-end">
          <label className="flex cursor-pointer items-center gap-2 text-[12px] text-text-muted">
            <input
              type="checkbox"
              checked={draft.stream}
              onChange={(e) => setDraft({ ...draft, stream: e.target.checked })}
              className="focus-ring h-3.5 w-3.5 cursor-pointer rounded accent-primary"
            />
            流式输出（降低首字延迟）
          </label>
        </div>
      </div>

      {/* 思考 / 非思考模式（ST-12） */}
      <div className="mt-4">
        <span className={LABEL}>回答模式</span>
        <div
          role="group"
          aria-label="回答模式"
          className="flex flex-wrap items-center gap-1.5"
        >
          {THINKING_OPTIONS.map((option) => (
            <button
              key={option.value}
              type="button"
              aria-pressed={draft.thinking === option.value}
              onClick={() => setDraft({ ...draft, thinking: option.value })}
              className={cn(
                "focus-ring cursor-pointer rounded-lg border px-3 py-1.5 text-[12px] transition-colors",
                draft.thinking === option.value
                  ? "border-primary/40 bg-primary/15 text-primary-3"
                  : "border-ghost/10 bg-background-3/60 text-text-muted hover:text-text"
              )}
            >
              {option.label}
            </button>
          ))}
          <span className="ml-1 text-[11px] text-text-subtle">{thinkingParamHint(activePreset)}</span>
        </div>
        <p className="mt-1.5 text-[11px] leading-relaxed text-text-subtle">
          非思考（默认）：直接作答，首字更快，适合面试实时场景。思考：先推理再作答，质量更高，但首字延迟明显增加（百炼上 qwen3.5 ~ qwen3.8 系列默认开启思考）。
        </p>
      </div>

      {/* API Key */}
      <div className="mt-4">
        <label className={LABEL} htmlFor="ai-api-key">
          API Key（保存到系统钥匙链，界面仅显示掩码）
        </label>
        <div className="flex items-center gap-2">
          <div className="flex min-w-0 flex-1 items-center gap-2">
            <KeyRound className="h-3.5 w-3.5 shrink-0 text-text-subtle" />
            <input
              id="ai-api-key"
              type="password"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder={
                apiKeyStatus?.configured
                  ? `已配置：${apiKeyStatus.masked}`
                  : "粘贴 API Key…"
              }
              className={FIELD}
            />
          </div>
          <button
            onClick={saveKey}
            disabled={!apiKey.trim()}
            className="focus-ring flex cursor-pointer items-center gap-1.5 rounded-lg border border-primary/40 bg-primary/15 px-3 py-2 text-[12px] text-primary-3 transition-colors hover:bg-primary/25 disabled:cursor-not-allowed disabled:opacity-40"
          >
            <Check className="h-3.5 w-3.5" />
            保存
          </button>
          <button
            onClick={() => onDeleteApiKey(draft.platform)}
            title="清除已保存的密钥"
            className="focus-ring flex cursor-pointer items-center gap-1.5 rounded-lg border border-ghost/10 bg-background-3/60 px-3 py-2 text-[12px] text-text-muted transition-colors hover:border-danger/40 hover:text-danger"
          >
            <Trash2 className="h-3.5 w-3.5" />
            清除
          </button>
        </div>
      </div>

      {testResult && (
        <div
          className={cn(
            "mt-2 flex items-center gap-2 rounded-md border px-3 py-2 text-[11px]",
            testResult.ok
              ? "border-success/30 bg-success/10 text-success"
              : "border-danger/30 bg-danger/10 text-danger"
          )}
        >
          <span className="min-w-0 flex-1">{testResult.message}</span>
          {testResult.ok && (
            <span className="shrink-0 opacity-80">{testResult.latencyMs} ms</span>
          )}
        </div>
      )}

      <div className="mt-4 flex items-center gap-3">
        {validationError && (
          <span className="flex items-center gap-1.5 text-[11px] text-danger">
            <AlertCircle className="h-3 w-3" />
            {validationError}
          </span>
        )}
        {!validationError && dirty && (
          <span className="text-[11px] text-warning">有未保存的修改</span>
        )}

        {onTestConnection && (
          <button
            onClick={onTestConnection}
            disabled={testing || Boolean(validationError)}
            className="focus-ring ml-auto flex cursor-pointer items-center gap-1.5 rounded-lg border border-ghost/10 bg-background-3/60 px-3 py-2 text-[12px] text-text-muted transition-colors hover:border-primary/40 hover:text-text disabled:cursor-not-allowed disabled:opacity-40"
          >
            {testing ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <Plug className="h-3.5 w-3.5" />
            )}
            {testing ? "测试中…" : "测试连接"}
          </button>
        )}

        <button
          onClick={() => onSaveConfig(draft)}
          disabled={Boolean(validationError)}
          className={cn(
            "focus-ring flex cursor-pointer items-center gap-1.5 rounded-lg border border-primary/40 bg-primary/15 px-4 py-2 text-[12px] text-primary-3 transition-colors hover:bg-primary/25 disabled:cursor-not-allowed disabled:opacity-40",
            !onTestConnection && "ml-auto"
          )}
        >
          <Save className="h-3.5 w-3.5" />
          保存配置
        </button>
        {/* 保存反馈：覆盖「保存配置」「保存 / 清除 API Key」 */}
        <SaveFeedback target="ai" />
      </div>
    </section>
  );
}
