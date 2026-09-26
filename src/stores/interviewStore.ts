import { create } from "zustand";
import { ipc } from "../lib/ipc";
import type { AudioDevice, CaptureStatus } from "../types";

/** 采集来源 */
export type CaptureSource = "mic" | "system";

interface InterviewState {
  status: CaptureStatus | null;
  /** 输入设备（麦克风 + 回环设备） */
  inputDevices: AudioDevice[];
  /** 当前电平（RMS 0~1） */
  level: number;
  error: string | null;
  starting: boolean;
  loadCaptureStatus: () => Promise<void>;
  startCapture: (source: CaptureSource, deviceId?: string | null) => Promise<void>;
  stopCapture: () => Promise<void>;
  applyLevel: (level: number) => void;
}

export const useInterviewStore = create<InterviewState>((set) => {
  const fail = (action: string, err: unknown) => {
    console.error(`${action}失败:`, err);
    set({ error: String(err), starting: false });
  };

  return {
    status: null,
    inputDevices: [],
    level: 0,
    error: null,
    starting: false,

    loadCaptureStatus: () =>
      Promise.all([
        ipc<CaptureStatus>("audio_capture_status"),
        ipc<AudioDevice[]>("list_audio_devices"),
      ])
        .then(([status, devices]) =>
          set({
            status,
            inputDevices: devices.filter((d) => d.kind === "input"),
          })
        )
        .catch((err) => fail("读取采集状态", err)),

    startCapture: (source, deviceId) => {
      set({ starting: true, error: null });
      return ipc<CaptureStatus>("start_audio_capture", {
        source,
        deviceId: deviceId ?? null,
      })
        .then((status) => set({ status, starting: false, level: 0 }))
        .catch((err) => fail("启动采集", err));
    },

    stopCapture: () => {
      set({ error: null });
      return ipc<CaptureStatus>("stop_audio_capture")
        .then((status) => set({ status, level: 0 }))
        .catch((err) => fail("停止采集", err));
    },

    applyLevel: (level) => set({ level }),
  };
});
