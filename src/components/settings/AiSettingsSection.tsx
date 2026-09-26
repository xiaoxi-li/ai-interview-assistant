import { useEffect, useState } from "react";
import { Check, KeyRound, Save, Trash2 } from "lucide-react";
import { cn } from "../../lib/utils";
import type { AIConfig, PlatformPreset } from "../../types";
import type { ApiKeyStatus } from "../../stores/settingsStore";

const FIELD =
  "w-full rounded-lg border border-white/10 bg-background-3/60 px-3 py-2 text-[13px] text-text outline-none transition-colors placeholder:text-text-subtle focus:border-primary/50";
const LABEL = "mb-1.5 block text-[11px] text-text-muted";

interface AiSettingsSectionProps {
  config: AIConfig;
  presets: PlatformPreset[];
  apiKeyStatus?: ApiKeyStatus;
  onSaveConfig: (config: AIConfig) => void;
  onSaveApiKey: (platform: string, apiKey: string) => void;
  onDeleteApiKey: (platform: string) => void;
}

export default function AiSettingsSection({
  config,
  presets,
  apiKeyStatus,
  onSaveConfig,
  onSaveApiKey,
  onDeleteApiKey,
}: AiSettingsSectionProps) {
  const [draft, setDraft] = useState<AIConfig>(config);
  const [apiKey, setApiKey] = useState("");

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
    <section className="glass rounded-xl p-4">
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
              "cursor-pointer rounded-lg border px-3 py-1.5 text-[12px] transition-colors",
              draft.platform === preset.platform
                ? "border-primary/40 bg-primary/15 text-primary-3"
                : "border-white/10 bg-background-3/60 text-text-muted hover:text-text"
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
            placeholder="https://api.deepseek.com/v1"
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
            placeholder="deepseek-chat"
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
            className="w-full cursor-pointer accent-[#4F6BED]"
          />
        </div>

        <div className="flex items-end">
          <label className="flex cursor-pointer items-center gap-2 text-[12px] text-text-muted">
            <input
              type="checkbox"
              checked={draft.stream}
              onChange={(e) => setDraft({ ...draft, stream: e.target.checked })}
              className="h-3.5 w-3.5 cursor-pointer accent-[#4F6BED]"
            />
            流式输出（降低首字延迟）
          </label>
        </div>
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
            className="flex cursor-pointer items-center gap-1.5 rounded-lg border border-primary/40 bg-primary/15 px-3 py-2 text-[12px] text-primary-3 transition-colors hover:bg-primary/25 disabled:cursor-not-allowed disabled:opacity-40"
          >
            <Check className="h-3.5 w-3.5" />
            保存
          </button>
          <button
            onClick={() => onDeleteApiKey(draft.platform)}
            title="清除已保存的密钥"
            className="flex cursor-pointer items-center gap-1.5 rounded-lg border border-white/10 bg-background-3/60 px-3 py-2 text-[12px] text-text-muted transition-colors hover:border-danger/40 hover:text-danger"
          >
            <Trash2 className="h-3.5 w-3.5" />
            清除
          </button>
        </div>
      </div>

      <div className="mt-4 flex justify-end">
        <button
          onClick={() => onSaveConfig(draft)}
          className="flex cursor-pointer items-center gap-1.5 rounded-lg border border-primary/40 bg-primary/15 px-4 py-2 text-[12px] text-primary-3 transition-colors hover:bg-primary/25"
        >
          <Save className="h-3.5 w-3.5" />
          保存配置
        </button>
      </div>
    </section>
  );
}
