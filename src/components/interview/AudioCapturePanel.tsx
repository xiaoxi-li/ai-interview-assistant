import { useEffect, useState } from "react";
import { Mic, Play, Square, Volume2, Waves } from "lucide-react";
import { cn } from "../../lib/utils";
import { useAudioLevel } from "../../hooks/useAudioLevel";
import { useInterviewStore, type CaptureSource } from "../../stores/interviewStore";

/** 电平条分段数量 */
const LEVEL_BARS = 24;

export default function AudioCapturePanel() {
  const status = useInterviewStore((s) => s.status);
  const inputDevices = useInterviewStore((s) => s.inputDevices);
  const systemDevices = useInterviewStore((s) => s.systemDevices);
  const level = useInterviewStore((s) => s.level);
  const starting = useInterviewStore((s) => s.starting);
  const error = useInterviewStore((s) => s.error);
  const loadCaptureStatus = useInterviewStore((s) => s.loadCaptureStatus);
  const startCapture = useInterviewStore((s) => s.startCapture);
  const stopCapture = useInterviewStore((s) => s.stopCapture);
  const applyLevel = useInterviewStore((s) => s.applyLevel);

  const [source, setSource] = useState<CaptureSource>("system");
  const [deviceId, setDeviceId] = useState<string>("");

  // 实时电平
  useAudioLevel(applyLevel);

  useEffect(() => {
    loadCaptureStatus();
  }, [loadCaptureStatus]);

  const running = status?.running ?? false;
  const activeBars = Math.round(level * LEVEL_BARS);
  // 系统音频的来源是「播放设备」（Windows 端点级回环）或虚拟回环设备，与麦克风列表不同
  const devices = source === "system" ? systemDevices : inputDevices;

  const handleToggle = () => {
    if (running) {
      stopCapture();
    } else {
      startCapture(source, deviceId || null);
    }
  };

  return (
    <section className="glass rounded-xl p-4 shadow-card">
      <header className="mb-3 flex items-center gap-2">
        <Waves className="h-4 w-4 text-primary-3" />
        <h2 className="text-[13px] font-semibold">音频采集</h2>
        <span
          className={cn(
            "ml-auto flex items-center gap-1.5 rounded-full px-2 py-0.5 text-[10px]",
            running
              ? "bg-success/15 text-success"
              : "bg-ghost/[0.06] text-text-subtle"
          )}
        >
          <span
            className={cn(
              "h-1.5 w-1.5 rounded-full",
              running ? "animate-pulse-dot bg-success" : "bg-text-subtle"
            )}
          />
          {running ? "采集中" : "未开始"}
        </span>
      </header>

      {/* 来源选择 */}
      <div className="mb-3 flex gap-1.5">
        {(
          [
            { key: "system" as const, label: "系统音频（面试官）", icon: Volume2 },
            { key: "mic" as const, label: "麦克风（我）", icon: Mic },
          ]
        ).map(({ key, label, icon: Icon }) => (
          <button
            key={key}
            disabled={running}
            onClick={() => setSource(key)}
            className={cn(
              "focus-ring flex flex-1 cursor-pointer items-center justify-center gap-1.5 rounded-lg border px-3 py-2 text-[12px] transition-colors disabled:cursor-not-allowed disabled:opacity-60",
              source === key
                ? "border-primary/40 bg-primary/15 text-primary-3"
                : "border-ghost/10 bg-background-3/60 text-text-muted hover:text-text"
            )}
          >
            <Icon className="h-3.5 w-3.5" />
            {label}
          </button>
        ))}
      </div>

      {/* 设备选择 */}
      <div className="mb-3">
        <select
          aria-label="采集设备"
          value={deviceId}
          disabled={running}
          onChange={(e) => setDeviceId(e.target.value)}
          className="focus-ring w-full cursor-pointer rounded-lg border border-ghost/10 bg-background-3/60 px-3 py-2 text-[12px] text-text outline-none focus:border-primary/50 disabled:cursor-not-allowed disabled:opacity-60"
        >
          <option value="">使用系统默认设备</option>
          {devices.map((device) => (
            <option key={device.id} value={device.id}>
              {device.name}
              {device.isDefault ? "（系统默认）" : ""}
            </option>
          ))}
        </select>
      </div>

      {/* 电平表 */}
      <div className="mb-3 flex items-center gap-2">
        <div className="flex h-6 flex-1 items-end gap-[2px]">
          {Array.from({ length: LEVEL_BARS }).map((_, index) => (
            <span
              key={index}
              className={cn(
                "h-full flex-1 rounded-sm transition-colors duration-75",
                index < activeBars
                  ? index > LEVEL_BARS * 0.8
                    ? "bg-danger"
                    : index > LEVEL_BARS * 0.5
                      ? "bg-warning"
                      : "bg-success"
                  : "bg-ghost/[0.06]"
              )}
            />
          ))}
        </div>
        <span className="w-10 shrink-0 text-right text-[10px] text-text-subtle">
          {Math.round(level * 100)}%
        </span>
      </div>

      {source === "system" && status && !status.loopbackAvailable && (
        <p className="mb-3 rounded-md border border-warning/30 bg-warning/10 px-3 py-2 text-[10px] leading-relaxed text-warning">
          未检测到可用的系统音频来源。Windows 需要至少一个可用的播放设备（直接在播放设备上
          回环采集，无需「立体声混音」）；macOS 需安装 BlackHole 等虚拟音频设备后重启应用。
        </p>
      )}

      {error && (
        <p className="mb-3 rounded-md border border-danger/30 bg-danger/10 px-3 py-2 text-[10px] text-danger">
          {error}
        </p>
      )}

      <p className="mt-2 text-[10px] leading-relaxed text-text-subtle">
        此处仅用于试听电平与确认设备，不产生转写文本；开始转写请在右侧「实时转写」中
        选择通道后点「开始转写」（转写与试听互斥，开始转写会停止试听）。
      </p>

      <button
        onClick={handleToggle}
        disabled={starting}
        className={cn(
          "focus-ring flex w-full cursor-pointer items-center justify-center gap-1.5 rounded-lg border px-4 py-2 text-[12px] transition-colors disabled:cursor-not-allowed disabled:opacity-50",
          running
            ? "border-danger/40 bg-danger/15 text-danger hover:bg-danger/25"
            : "border-primary/40 bg-primary/15 text-primary-3 hover:bg-primary/25"
        )}
      >
        {running ? <Square className="h-3.5 w-3.5" /> : <Play className="h-3.5 w-3.5" />}
        {running ? "停止采集" : "开始采集"}
      </button>
    </section>
  );
}
