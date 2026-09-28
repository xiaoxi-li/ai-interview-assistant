import { useEffect, useMemo, useState } from "react";
import {
  AlertCircle,
  Check,
  HardDrive,
  PlugZap,
  RotateCcw,
  Save,
  Trash2,
  UploadCloud,
} from "lucide-react";
import { cn } from "../../lib/utils";
import {
  activeAsrModel,
  baseUrlWarning,
  credentialFieldLabel,
  effectiveBaseUrl,
  hasCustomBaseUrl,
  missingCredentials,
  validateAsrConfig,
  withProviderBaseUrl,
} from "../../lib/asrConfig";
import type {
  ASRConfig,
  AsrCredentialStatus,
  AsrProviderPreset,
  ConnectionTest,
} from "../../types";

const FIELD =
  "focus-ring w-full rounded-lg border border-white/10 bg-background-3/60 px-3 py-2 text-[13px] text-text outline-none transition-colors placeholder:text-text-subtle focus:border-primary/50";
const LABEL = "mb-1.5 block text-[11px] text-text-muted";

interface AsrSettingsSectionProps {
  config: ASRConfig;
  providers: AsrProviderPreset[];
  credentialStatus?: AsrCredentialStatus;
  onSaveConfig: (config: ASRConfig) => void;
  onSaveCredential: (provider: string, field: string, value: string) => void;
  onDeleteCredentials: (provider: string) => void;
  onProviderChange?: (provider: string) => void;
  /** 云端 ASR 连接测试（IA-16.8）：回传当前表单草稿，避免测到未保存的旧配置 */
  onTestConnection?: (config: ASRConfig) => void;
  testing?: boolean;
  testResult?: ConnectionTest | null;
}

export default function AsrSettingsSection({
  config,
  providers,
  credentialStatus,
  onSaveConfig,
  onSaveCredential,
  onDeleteCredentials,
  onProviderChange,
  onTestConnection,
  testing = false,
  testResult = null,
}: AsrSettingsSectionProps) {
  const [draft, setDraft] = useState<ASRConfig>(config);
  const [credentialInputs, setCredentialInputs] = useState<Record<string, string>>({});

  useEffect(() => {
    setDraft(config);
  }, [config]);

  const preset = providers.find((p) => p.provider === draft.cloudProvider);
  const validationError = useMemo(() => validateAsrConfig(draft), [draft]);
  const customBaseUrl = hasCustomBaseUrl(draft, draft.cloudProvider);
  const effectiveUrl = effectiveBaseUrl(
    draft,
    draft.cloudProvider,
    preset?.defaultBaseUrl ?? ""
  );
  const baseUrlNotice = baseUrlWarning(
    draft.cloudBaseUrls?.[draft.cloudProvider],
    preset?.usesWebsocket ?? true
  );

  const configuredFields = useMemo(
    () =>
      (credentialStatus?.fields ?? [])
        .filter((f) => f.configured)
        .map((f) => f.field),
    [credentialStatus]
  );
  const missing = missingCredentials(draft, configuredFields);

  const saveCredential = (field: string) => {
    const value = credentialInputs[field]?.trim();
    if (!value) return;
    onSaveCredential(draft.cloudProvider, field, value);
    setCredentialInputs((prev) => ({ ...prev, [field]: "" }));
  };

  return (
    <section className="glass rounded-xl p-4 shadow-card">
      <header className="mb-4 flex items-center gap-2">
        <h2 className="text-[13px] font-semibold">语音识别（ASR）</h2>
        <span className="text-[11px] text-text-subtle">
          面试官语音的实时转写来源
        </span>
      </header>

      {/* 运行模式 */}
      <div className="mb-4 flex gap-1.5">
        {(
          [
            { mode: "cloud" as const, label: "云端（低延迟）", icon: UploadCloud },
            { mode: "local" as const, label: "本地（隐私优先）", icon: HardDrive },
          ]
        ).map(({ mode, label, icon: Icon }) => (
          <button
            key={mode}
            onClick={() => setDraft({ ...draft, mode })}
            className={cn(
              "focus-ring flex flex-1 cursor-pointer items-center justify-center gap-1.5 rounded-lg border px-3 py-2 text-[12px] transition-colors",
              draft.mode === mode
                ? "border-primary/40 bg-primary/15 text-primary-3"
                : "border-white/10 bg-background-3/60 text-text-muted hover:text-text"
            )}
          >
            <Icon className="h-3.5 w-3.5" />
            {label}
          </button>
        ))}
      </div>

      {draft.mode === "cloud" ? (
        <>
          {/* 服务商 */}
          <div className="mb-3 flex flex-wrap gap-1.5">
            {providers.map((provider) => (
              <button
                key={provider.provider}
                onClick={() => {
                  setDraft({
                    ...draft,
                    cloudProvider: provider.provider,
                    model: provider.defaultModel,
                  });
                  onProviderChange?.(provider.provider);
                }}
                className={cn(
                  "focus-ring flex cursor-pointer items-center gap-1.5 rounded-lg border px-3 py-1.5 text-[12px] transition-colors",
                  draft.cloudProvider === provider.provider
                    ? "border-primary/40 bg-primary/15 text-primary-3"
                    : "border-white/10 bg-background-3/60 text-text-muted hover:text-text"
                )}
              >
                {provider.label}
                <span
                  className={cn(
                    "rounded px-1 py-0.5 text-[10px] leading-none",
                    provider.implemented
                      ? "bg-success/15 text-success"
                      : "bg-white/[0.06] text-text-subtle"
                  )}
                >
                  {provider.implemented ? "已接入" : "待接入"}
                </span>
              </button>
            ))}
          </div>

          <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
            <div>
              <label className={LABEL} htmlFor="asr-model">
                云端模型
              </label>
              <input
                id="asr-model"
                value={draft.model}
                onChange={(e) => setDraft({ ...draft, model: e.target.value })}
                placeholder="iat"
                className={FIELD}
              />
              {draft.cloudProvider === "aliyun" && (
                <p className="mt-1 text-[10px] leading-relaxed text-text-subtle">
                  实时模型示例：qwen-audio-3.1-asr-flash-streaming、fun-asr-realtime、
                  paraformer-realtime-v2；非实时（HTTP）模型如
                  qwen-audio-3.1-asr-flash 不能用于实时转写。
                </p>
              )}
            </div>

            <div>
              <label className={LABEL} htmlFor="asr-base-url">
                服务端地址（Base URL）
              </label>
              <div className="flex items-center gap-2">
                <input
                  id="asr-base-url"
                  value={draft.cloudBaseUrls?.[draft.cloudProvider] ?? ""}
                  onChange={(e) =>
                    setDraft(
                      withProviderBaseUrl(draft, draft.cloudProvider, e.target.value)
                    )
                  }
                  placeholder={preset?.defaultBaseUrl ?? "wss://example.com/ws"}
                  className={FIELD}
                />
                <button
                  onClick={() =>
                    setDraft(withProviderBaseUrl(draft, draft.cloudProvider, ""))
                  }
                  disabled={!customBaseUrl}
                  className="focus-ring flex shrink-0 cursor-pointer items-center gap-1.5 rounded-lg border border-white/10 bg-background-3/60 px-2.5 py-2 text-[11px] text-text-muted transition-colors hover:text-text disabled:cursor-not-allowed disabled:opacity-40"
                >
                  <RotateCcw className="h-3 w-3" />
                  恢复默认
                </button>
              </div>
              <p className="mt-1 text-[10px] leading-relaxed text-text-subtle">
                {customBaseUrl
                  ? `当前使用自定义地址；官方默认：${preset?.defaultBaseUrl ?? "—"}`
                  : "当前使用官方默认地址；可填写代理或私有化部署地址，各服务商各自保存"}
              </p>
              {baseUrlNotice && (
                <p className="mt-1 rounded-md border border-warning/30 bg-warning/10 px-2 py-1.5 text-[10px] leading-relaxed text-warning">
                  {baseUrlNotice}
                </p>
              )}
            </div>
          </div>

          {preset && !preset.implemented && (
            <p className="mt-3 rounded-md border border-warning/30 bg-warning/10 px-3 py-2 text-[11px] leading-relaxed text-warning">
              「{preset.label}」协议待接入：配置与凭证可正常保存，但开始转写会提示切换到「阿里云智能语音」或本地模式。
            </p>
          )}

          <p className="mt-2 text-[10px] text-text-subtle">
            当前生效地址：{effectiveUrl || "—"}
          </p>

          {/* 凭证 */}
          <div className="mt-4">
            <div className="mb-1.5 flex items-center gap-2">
              <label className={cn(LABEL, "mb-0")}>服务商凭证（存系统钥匙链）</label>
              {missing.length > 0 && (
                <span className="text-[10px] text-warning">
                  缺少 {missing.map(credentialFieldLabel).join("、")}
                </span>
              )}
            </div>
            <div className="flex flex-col gap-2">
              {(preset?.credentialFields ?? []).map((field) => {
                const status = credentialStatus?.fields.find((f) => f.field === field);
                return (
                  <div key={field} className="flex items-center gap-2">
                    <span className="w-20 shrink-0 text-[11px] text-text-muted">
                      {credentialFieldLabel(field)}
                    </span>
                    <input
                      type="password"
                      aria-label={credentialFieldLabel(field)}
                      value={credentialInputs[field] ?? ""}
                      onChange={(e) =>
                        setCredentialInputs((prev) => ({ ...prev, [field]: e.target.value }))
                      }
                      placeholder={
                        status?.configured ? `已配置：${status.masked}` : "未配置"
                      }
                      className={FIELD}
                    />
                    <button
                      onClick={() => saveCredential(field)}
                      disabled={!(credentialInputs[field] ?? "").trim()}
                      className="focus-ring flex cursor-pointer items-center gap-1.5 rounded-lg border border-primary/40 bg-primary/15 px-3 py-2 text-[12px] text-primary-3 transition-colors hover:bg-primary/25 disabled:cursor-not-allowed disabled:opacity-40"
                    >
                      <Check className="h-3.5 w-3.5" />
                      保存
                    </button>
                  </div>
                );
              })}
            </div>
            <button
              onClick={() => onDeleteCredentials(draft.cloudProvider)}
              className="focus-ring mt-2 flex cursor-pointer items-center gap-1.5 rounded-lg border border-white/10 bg-background-3/60 px-3 py-1.5 text-[11px] text-text-muted transition-colors hover:border-danger/40 hover:text-danger"
            >
              <Trash2 className="h-3 w-3" />
              清空该服务商全部凭证
            </button>
          </div>
        </>
      ) : (
        <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
          <div>
            <label className={LABEL} htmlFor="asr-local-model">
              本地模型
            </label>
            <input
              id="asr-local-model"
              value={draft.localModel}
              onChange={(e) => setDraft({ ...draft, localModel: e.target.value })}
              placeholder="sherpa-onnx-paraformer-zh"
              className={FIELD}
            />
            <p className="mt-1 text-[10px] text-text-subtle">
              本地模型需在「模型管理」中下载后使用，识别过程不上传音频
            </p>
          </div>
        </div>
      )}

      <div className="mt-4 grid grid-cols-1 gap-3 lg:grid-cols-2">
        <div>
          <label className={LABEL} htmlFor="asr-language">
            识别语言
          </label>
          <input
            id="asr-language"
            value={draft.language}
            onChange={(e) => setDraft({ ...draft, language: e.target.value })}
            placeholder="zh-CN"
            className={FIELD}
          />
        </div>

        <div>
          <label className={LABEL} htmlFor="asr-vad">
            VAD 阈值：{draft.vadThreshold.toFixed(2)}
          </label>
          <input
            id="asr-vad"
            type="range"
            min={0}
            max={1}
            step={0.05}
            value={draft.vadThreshold}
            onChange={(e) =>
              setDraft({ ...draft, vadThreshold: Number(e.target.value) })
            }
            className="w-full cursor-pointer accent-[#4F6BED]"
          />
          <p className="mt-1 text-[10px] leading-relaxed text-text-subtle">
            值越大越严格（0.5 约等于 -33dBFS，适合正常说话音量）。识别不到说话时调低，
            环境噪声误触发时调高。
          </p>
        </div>
      </div>

      <div className="mt-4 flex items-center gap-3">
        <span className="text-[11px] text-text-subtle">
          当前生效模型：{activeAsrModel(draft)}
        </span>
        {validationError && (
          <span className="flex items-center gap-1.5 text-[11px] text-danger">
            <AlertCircle className="h-3 w-3" />
            {validationError}
          </span>
        )}
        {draft.mode === "cloud" && (
          <button
            onClick={() => onTestConnection?.(draft)}
            disabled={testing || Boolean(validationError)}
            className="focus-ring ml-auto flex cursor-pointer items-center gap-1.5 rounded-lg border border-white/10 bg-background-3/60 px-3 py-2 text-[12px] text-text-muted transition-colors hover:text-text disabled:cursor-not-allowed disabled:opacity-40"
          >
            <PlugZap className="h-3.5 w-3.5" />
            {testing ? "测试中…" : "连接测试"}
          </button>
        )}
        <button
          onClick={() => onSaveConfig(draft)}
          disabled={Boolean(validationError)}
          className={cn(
            "focus-ring flex cursor-pointer items-center gap-1.5 rounded-lg border border-primary/40 bg-primary/15 px-4 py-2 text-[12px] text-primary-3 transition-colors hover:bg-primary/25 disabled:cursor-not-allowed disabled:opacity-40",
            draft.mode !== "cloud" && "ml-auto"
          )}
        >
          <Save className="h-3.5 w-3.5" />
          保存配置
        </button>
      </div>

      {draft.mode === "cloud" && (
        <p className="mt-2 text-[10px] text-text-subtle">
          「连接测试」使用当前表单值（含未保存的修改），不需要先点保存。
        </p>
      )}

      {testResult && (
        <p
          className={cn(
            "mt-2 flex items-start gap-1.5 text-[11px]",
            testResult.ok ? "text-success" : "text-danger"
          )}
        >
          {testResult.ok ? (
            <Check className="mt-[1px] h-3 w-3 shrink-0" />
          ) : (
            <AlertCircle className="mt-[1px] h-3 w-3 shrink-0" />
          )}
          <span className="leading-relaxed">{testResult.message}</span>
        </p>
      )}
    </section>
  );
}
