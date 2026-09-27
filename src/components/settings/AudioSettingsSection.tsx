import { useEffect, useState } from "react";
import { Mic, RefreshCw, Save, Volume2 } from "lucide-react";
import { cn } from "../../lib/utils";
import type { AudioConfig, AudioDevice } from "../../types";

const FIELD =
  "focus-ring w-full cursor-pointer rounded-lg border border-white/10 bg-background-3/60 px-3 py-2 text-[13px] text-text outline-none transition-colors focus:border-primary/50";
const LABEL = "mb-1.5 block text-[11px] text-text-muted";

interface AudioSettingsSectionProps {
  config: AudioConfig;
  devices: AudioDevice[];
  onRefresh: () => void;
  onSave: (config: AudioConfig) => void;
}

/** 将可选设备渲染为下拉项，默认设备加标注 */
function DeviceSelect({
  id,
  label,
  icon: Icon,
  value,
  devices,
  placeholder,
  onChange,
}: {
  id: string;
  label: string;
  icon: typeof Mic;
  value: string | null;
  devices: AudioDevice[];
  placeholder: string;
  onChange: (value: string | null) => void;
}) {
  return (
    <div>
      <label className={LABEL} htmlFor={id}>
        <span className="inline-flex items-center gap-1.5">
          <Icon className="h-3 w-3" />
          {label}
        </span>
      </label>
      <select
        id={id}
        value={value ?? ""}
        onChange={(e) => onChange(e.target.value ? e.target.value : null)}
        className={FIELD}
      >
        <option value="">{placeholder}</option>
        {devices.map((device) => (
          <option key={device.id} value={device.id}>
            {device.name}
            {device.isDefault ? "（系统默认）" : ""}
          </option>
        ))}
      </select>
    </div>
  );
}

export default function AudioSettingsSection({
  config,
  devices,
  onRefresh,
  onSave,
}: AudioSettingsSectionProps) {
  const [draft, setDraft] = useState<AudioConfig>(config);

  useEffect(() => {
    setDraft(config);
  }, [config]);

  const inputs = devices.filter((d) => d.kind === "input");
  const outputs = devices.filter((d) => d.kind === "output");

  return (
    <section className="glass rounded-xl p-4 shadow-card">
      <header className="mb-4 flex items-center gap-2">
        <h2 className="text-[13px] font-semibold">音频设备</h2>
        <span className="text-[11px] text-text-subtle">
          麦克风捕获「我」，系统音频捕获面试官
        </span>
        <button
          onClick={onRefresh}
          title="重新枚举设备"
          className="focus-ring ml-auto flex cursor-pointer items-center gap-1.5 rounded-lg border border-white/10 bg-background-3/60 px-2.5 py-1.5 text-[11px] text-text-muted transition-colors hover:border-primary/40 hover:text-text"
        >
          <RefreshCw className="h-3 w-3" />
          刷新设备
        </button>
      </header>

      <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
        <DeviceSelect
          id="mic-device"
          label="麦克风（我的声音）"
          icon={Mic}
          value={draft.micDeviceId}
          devices={inputs}
          placeholder="使用系统默认麦克风"
          onChange={(micDeviceId) => setDraft({ ...draft, micDeviceId })}
        />
        <DeviceSelect
          id="system-device"
          label="系统音频（面试官声音）"
          icon={Volume2}
          value={draft.systemDeviceId}
          devices={outputs}
          placeholder="使用系统默认输出设备"
          onChange={(systemDeviceId) => setDraft({ ...draft, systemDeviceId })}
        />
      </div>

      <div className="mt-3 flex items-center gap-3">
        <span
          className={cn(
            "text-[10px]",
            devices.length === 0 ? "text-warning" : "text-text-subtle"
          )}
        >
          {devices.length === 0
            ? "未检测到音频设备，请检查系统音频驱动或连接麦克风"
            : `已检测到 ${inputs.length} 个输入 / ${outputs.length} 个输出设备`}
        </span>
        <button
          onClick={() => onSave(draft)}
          className="focus-ring ml-auto flex cursor-pointer items-center gap-1.5 rounded-lg border border-primary/40 bg-primary/15 px-4 py-2 text-[12px] text-primary-3 transition-colors hover:bg-primary/25"
        >
          <Save className="h-3.5 w-3.5" />
          保存配置
        </button>
      </div>
    </section>
  );
}
