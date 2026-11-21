import { useCallback, useEffect, useState } from "react";
import { AppWindow, Mic, RefreshCw, Save, Volume2 } from "lucide-react";
import { cn } from "../../lib/utils";
import { ipc } from "../../lib/ipc";
import { SaveFeedback } from "./SaveFeedback";
import type { AudioConfig, AudioDevice, AudioProcess } from "../../types";

const FIELD =
  "focus-ring w-full cursor-pointer rounded-lg border border-ghost/10 bg-background-3/60 px-3 py-2 text-[13px] text-text outline-none transition-colors focus:border-primary/50";
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
  // TODO-05：进程级回环的目标进程列表（本地加载，仅选择时需要）
  const [processes, setProcesses] = useState<AudioProcess[]>([]);
  const [processesLoading, setProcessesLoading] = useState(false);

  useEffect(() => {
    setDraft(config);
  }, [config]);

  const loadProcesses = useCallback(() => {
    setProcessesLoading(true);
    ipc<AudioProcess[]>("list_audio_processes")
      .then((list) => setProcesses(list))
      .catch((err) => console.error("读取音频进程列表失败:", err))
      .finally(() => setProcessesLoading(false));
  }, []);

  const processTarget = draft.processLoopbackPid ?? null;
  /** 进程模式：已选目标或用户显式切换（pending）；缺省字段视为端点模式 */
  const [processPending, setProcessPending] = useState(false);
  const processMode = processPending || processTarget !== null;
  // 挂载时若已配置进程目标，拉一次进程列表以回显名称
  useEffect(() => {
    if (processTarget !== null && processes.length === 0) {
      loadProcesses();
    }
  }, [processTarget, processes.length, loadProcesses]);

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
          className="focus-ring ml-auto flex cursor-pointer items-center gap-1.5 rounded-lg border border-ghost/10 bg-background-3/60 px-2.5 py-1.5 text-[11px] text-text-muted transition-colors hover:border-primary/40 hover:text-text"
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

      {/* 进程级回环（TODO-05 / IA-14）：只采集指定应用的音频 */}
      <div className="mt-4 rounded-lg border border-ghost/[0.06] bg-background-3/40 p-3">
        <label className={LABEL}>
          <span className="inline-flex items-center gap-1.5">
            <AppWindow className="h-3 w-3" />
            系统音频采集范围
          </span>
        </label>
        <div className="flex gap-1.5">
          <button
            type="button"
            aria-pressed={!processMode}
            onClick={() => {
              setProcessPending(false);
              setDraft({ ...draft, processLoopbackPid: null });
            }}
            className={cn(
              "focus-ring flex-1 cursor-pointer rounded-lg border px-3 py-1.5 text-[12px] transition-colors",
              !processMode
                ? "border-primary/40 bg-primary/15 text-primary-3"
                : "border-ghost/10 bg-background-3/60 text-text-muted hover:text-text"
            )}
          >
            全部应用（混音采集）
          </button>
          <button
            type="button"
            aria-pressed={processMode}
            onClick={() => {
              if (!processMode) {
                // 首次切换到进程模式：进入待选状态并加载应用列表
                setProcessPending(true);
                loadProcesses();
              }
            }}
            className={cn(
              "focus-ring flex-1 cursor-pointer rounded-lg border px-3 py-1.5 text-[12px] transition-colors",
              processMode
                ? "border-primary/40 bg-primary/15 text-primary-3"
                : "border-ghost/10 bg-background-3/60 text-text-muted hover:text-text"
            )}
          >
            只采集指定应用
          </button>
        </div>

        {processMode && (
          <div className="mt-2 flex items-center gap-1.5">
            <select
              aria-label="目标应用"
              value={processTarget ?? ""}
              onChange={(e) =>
                setDraft({
                  ...draft,
                  processLoopbackPid: e.target.value ? Number(e.target.value) : undefined,
                })
              }
              className={cn(FIELD, "min-w-0 flex-1")}
            >
              {processes.length === 0 && (
                <option value="">未检测到正在发声的应用</option>
              )}
              {processes.map((process) => (
                <option key={process.pid} value={process.pid}>
                  {process.name}（pid {process.pid}）{process.active ? " · 正在发声" : ""}
                </option>
              ))}
            </select>
            <button
              type="button"
              aria-label="刷新应用列表"
              title="刷新正在发声的应用列表"
              onClick={loadProcesses}
              disabled={processesLoading}
              className="focus-ring flex h-[38px] shrink-0 cursor-pointer items-center justify-center rounded-lg border border-ghost/10 bg-background-3/60 px-2.5 text-text-muted transition-colors hover:text-text disabled:cursor-not-allowed disabled:opacity-50"
            >
              <RefreshCw className={cn("h-3.5 w-3.5", processesLoading && "animate-spin")} />
            </button>
          </div>
        )}
        <p className="mt-1.5 text-[10px] leading-relaxed text-text-subtle">
          「只采集指定应用」通过 WASAPI 进程级回环只录选中应用（含其子进程，如会议软件）
          的声音，背景音乐不会被混入面试官通道。需要 Windows 10 2004 及以上；目标应用需
          正在播放音频。
        </p>
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
          onClick={() =>
            onSave({ ...draft, processLoopbackPid: draft.processLoopbackPid ?? null })
          }
          className="focus-ring ml-auto flex cursor-pointer items-center gap-1.5 rounded-lg border border-primary/40 bg-primary/15 px-4 py-2 text-[12px] text-primary-3 transition-colors hover:bg-primary/25"
        >
          <Save className="h-3.5 w-3.5" />
          保存配置
        </button>
        {/* 保存反馈：覆盖「保存配置」「刷新设备」 */}
        <SaveFeedback target="audio" />
      </div>
    </section>
  );
}
