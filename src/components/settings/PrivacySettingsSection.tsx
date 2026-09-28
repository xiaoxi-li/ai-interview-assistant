import { useEffect, useState } from "react";
import { AlertTriangle, Loader2, Lock, Save, Trash2, UploadCloud } from "lucide-react";
import { cn } from "../../lib/utils";
import { SaveFeedback } from "./SaveFeedback";
import type { PrivacyConfig } from "../../types";

interface PrivacySettingsSectionProps {
  config: PrivacyConfig;
  /** 当前是否使用云端 ASR（决定上传开关是否可用） */
  cloudAsrEnabled: boolean;
  clearing: boolean;
  onSave: (config: PrivacyConfig) => void;
  onClearData: () => void;
}

function SwitchRow({
  id,
  icon: Icon,
  title,
  desc,
  checked,
  disabled,
  warning,
  onChange,
}: {
  id: string;
  icon: typeof Lock;
  title: string;
  desc: string;
  checked: boolean;
  disabled?: boolean;
  warning?: string;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label
      htmlFor={id}
      className={cn(
        "flex cursor-pointer items-start gap-3 rounded-md border border-ghost/[0.06] bg-background-3/40 p-3 transition-colors",
        disabled ? "cursor-not-allowed opacity-60" : "hover:border-primary/30"
      )}
    >
      <Icon className="mt-0.5 h-4 w-4 shrink-0 text-text-muted" />
      <span className="min-w-0 flex-1">
        <span className="block text-[12px] font-medium text-text">{title}</span>
        <span className="mt-0.5 block text-[11px] leading-relaxed text-text-muted">
          {desc}
        </span>
        {warning && (
          <span className="mt-1 flex items-center gap-1 text-[10px] text-warning">
            <AlertTriangle className="h-3 w-3" />
            {warning}
          </span>
        )}
      </span>
      <input
        id={id}
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange(e.target.checked)}
        className="focus-ring mt-0.5 h-4 w-4 shrink-0 cursor-pointer rounded accent-primary"
      />
    </label>
  );
}

export default function PrivacySettingsSection({
  config,
  cloudAsrEnabled,
  clearing,
  onSave,
  onClearData,
}: PrivacySettingsSectionProps) {
  const [draft, setDraft] = useState<PrivacyConfig>(config);
  const [confirming, setConfirming] = useState(false);

  useEffect(() => {
    setDraft(config);
  }, [config]);

  return (
    <section className="glass rounded-xl p-4 shadow-card">
      <header className="mb-4 flex items-center gap-2">
        <h2 className="text-[13px] font-semibold">隐私与安全</h2>
        <span className="text-[11px] text-text-subtle">
          数据默认仅保存在本机
        </span>
      </header>

      <div className="flex flex-col gap-2">
        <SwitchRow
          id="privacy-upload-audio"
          icon={UploadCloud}
          title="允许上传音频至云端 ASR"
          desc="开启后面试音频将发送至所选云端服务商进行转写；本地 ASR 模式下无需上传。"
          checked={draft.uploadAudio}
          disabled={!cloudAsrEnabled}
          warning={
            cloudAsrEnabled
              ? undefined
              : "当前为本地 ASR 模式，如需上传请先在 ASR 设置中切换为云端"
          }
          onChange={(uploadAudio) => setDraft({ ...draft, uploadAudio })}
        />

        <SwitchRow
          id="privacy-encrypt-local"
          icon={Lock}
          title="本地敏感数据加密"
          desc="对转写正文与问答内容进行字段级加密（AES-GCM），密钥由系统钥匙链保护。"
          checked={draft.encryptLocal}
          onChange={(encryptLocal) => setDraft({ ...draft, encryptLocal })}
        />
      </div>

      <div className="mt-3 flex items-center gap-3">
        <button
          onClick={() => onSave(draft)}
          className="focus-ring flex cursor-pointer items-center gap-1.5 rounded-lg border border-primary/40 bg-primary/15 px-4 py-2 text-[12px] text-primary-3 transition-colors hover:bg-primary/25"
        >
          <Save className="h-3.5 w-3.5" />
          保存配置
        </button>
        {/* 保存反馈：隐私与安全配置保存 */}
        <SaveFeedback target="privacy" />
      </div>

      {/* 数据清理 */}
      <div className="mt-4 rounded-md border border-danger/25 bg-danger/[0.06] p-3">
        <div className="flex items-center gap-2">
          <Trash2 className="h-4 w-4 text-danger" />
          <span className="text-[12px] font-medium text-danger">清除本地数据</span>
        </div>
        <p className="mt-1 text-[11px] leading-relaxed text-text-muted">
          删除全部笔记、群组、标签与面试会话记录及检索索引；应用设置与钥匙链凭证将保留，操作不可撤销。
        </p>

        {confirming ? (
          <div className="mt-2 flex items-center gap-2">
            <span className="text-[11px] text-danger">确认清除全部本地数据？</span>
            <button
              onClick={() => {
                onClearData();
                setConfirming(false);
              }}
              disabled={clearing}
              className="focus-ring flex cursor-pointer items-center gap-1.5 rounded-lg border border-danger/50 bg-danger/20 px-3 py-1.5 text-[11px] text-danger transition-colors hover:bg-danger/30 disabled:cursor-not-allowed disabled:opacity-50"
            >
              {clearing && <Loader2 className="h-3 w-3 animate-spin" />}
              确认清除
            </button>
            <button
              onClick={() => setConfirming(false)}
              className="focus-ring cursor-pointer rounded-lg border border-ghost/10 px-3 py-1.5 text-[11px] text-text-muted transition-colors hover:text-text"
            >
              取消
            </button>
          </div>
        ) : (
          <button
            onClick={() => setConfirming(true)}
            className="focus-ring mt-2 cursor-pointer rounded-lg border border-danger/40 px-3 py-1.5 text-[11px] text-danger transition-colors hover:bg-danger/15"
          >
            清除数据…
          </button>
        )}
      </div>
    </section>
  );
}
