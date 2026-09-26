import { create } from "zustand";
import { ipc } from "../lib/ipc";
import type {
  AsrEngineInfo,
  AsrSessionStatus,
  AudioDevice,
  CaptureStatus,
  DetectedQuestion,
  QaPair,
  QuestionConfig,
  QuestionPush,
  Speaker,
  TranscriptPush,
} from "../types";

/** 采集来源 */
export type CaptureSource = "mic" | "system";

/** 界面展示的转写行 */
export interface TranscriptLine {
  id: string;
  speaker: Speaker;
  text: string;
  isFinal: boolean;
  ts: number;
}

interface InterviewState {
  status: CaptureStatus | null;
  /** 输入设备（麦克风 + 回环设备） */
  inputDevices: AudioDevice[];
  /** 当前电平（RMS 0~1） */
  level: number;
  error: string | null;
  starting: boolean;

  /** 本地 ASR 会话状态 */
  asrStatus: AsrSessionStatus | null;
  /** 可用本地 ASR 引擎 */
  engines: AsrEngineInfo[];
  /** 已确认的转写（final） */
  transcript: TranscriptLine[];
  /** 当前进行中的中间结果（partial） */
  partial: TranscriptLine | null;
  /** 转写错误（如模型未下载） */
  asrError: string | null;
  startingAsr: boolean;

  /** 最近识别出的问题（IA-06） */
  currentQuestion: DetectedQuestion | null;
  /** 最近一次问题是否为手动触发 */
  questionManual: boolean;
  /** 会话内已识别的问题（落库记录） */
  questions: QaPair[];
  /** 问题识别配置 */
  questionConfig: QuestionConfig | null;

  loadCaptureStatus: () => Promise<void>;
  startCapture: (source: CaptureSource, deviceId?: string | null) => Promise<void>;
  stopCapture: () => Promise<void>;
  applyLevel: (level: number) => void;

  loadAsrStatus: () => Promise<void>;
  startTranscription: (source: CaptureSource, deviceId?: string | null) => Promise<void>;
  stopTranscription: () => Promise<void>;
  applyTranscriptPush: (push: TranscriptPush) => void;
  clearTranscript: () => void;

  applyQuestionPush: (push: QuestionPush) => void;
  triggerQuestion: () => Promise<void>;
  loadQuestions: (sessionId: string) => Promise<void>;
  loadQuestionConfig: () => Promise<void>;
  saveQuestionConfig: (config: QuestionConfig) => Promise<void>;
  resetQuestions: () => void;
}

export const useInterviewStore = create<InterviewState>((set, get) => {
  const fail = (action: string, err: unknown) => {
    console.error(`${action}失败:`, err);
    set({ error: String(err), starting: false, startingAsr: false });
  };

  return {
    status: null,
    inputDevices: [],
    level: 0,
    error: null,
    starting: false,

    asrStatus: null,
    engines: [],
    transcript: [],
    partial: null,
    asrError: null,
    startingAsr: false,

    currentQuestion: null,
    questionManual: false,
    questions: [],
    questionConfig: null,

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

    loadAsrStatus: () =>
      Promise.all([
        ipc<AsrSessionStatus>("asr_session_status"),
        ipc<AsrEngineInfo[]>("list_asr_engines"),
      ])
        .then(([asrStatus, engines]) => set({ asrStatus, engines }))
        .catch((err) => fail("读取转写状态", err)),

    startTranscription: (source, deviceId) => {
      set({ startingAsr: true, asrError: null, error: null });
      return ipc<AsrSessionStatus>("start_asr_session", {
        source,
        deviceId: deviceId ?? null,
      })
        .then((asrStatus) =>
          set({
            asrStatus,
            startingAsr: false,
            partial: null,
            // 新会话：清空上一场残留的转写与问题
            transcript: [],
            currentQuestion: null,
            questionManual: false,
            questions: [],
          })
        )
        .catch((err) => {
          console.error("启动转写失败:", err);
          set({ startingAsr: false, asrError: String(err) });
        });
    },

    stopTranscription: () => {
      set({ asrError: null });
      return ipc<AsrSessionStatus>("stop_asr_session")
        .then((asrStatus) => set({ asrStatus, partial: null }))
        .catch((err) => fail("停止转写", err));
    },

    applyTranscriptPush: (push) => {
      const { event, ts } = push;

      if (event.kind === "error") {
        set({ asrError: event.message });
        return;
      }

      const line: TranscriptLine = {
        id: event.kind === "partial" ? "partial" : `${ts}-${get().transcript.length}`,
        speaker: event.speaker,
        text: event.text,
        isFinal: event.kind === "final",
        ts,
      };

      if (event.kind === "partial") {
        set({ partial: line });
      } else {
        set((state) => ({ transcript: [...state.transcript, line], partial: null }));
      }
    },

    clearTranscript: () => set({ transcript: [], partial: null, asrError: null }),

    applyQuestionPush: (push) => {
      const { question, manual, qaId, sessionId } = push;
      set((state) => ({
        currentQuestion: question,
        questionManual: manual,
        questions: qaId
          ? [
              ...state.questions,
              {
                id: qaId,
                sessionId,
                question: question.text,
                answer: "",
                sources: [],
                ts: question.ts,
              },
            ]
          : state.questions,
      }));
    },

    triggerQuestion: () => {
      return ipc<void>("trigger_question").catch((err) => {
        console.error("手动识别问题失败:", err);
        set({ asrError: String(err) });
      });
    },

    loadQuestions: (sessionId) => {
      return ipc<QaPair[]>("list_questions", { sessionId, limit: 100 })
        .then((questions) => set({ questions }))
        .catch((err) => fail("读取问题记录", err));
    },

    loadQuestionConfig: () => {
      return ipc<QuestionConfig>("get_question_config")
        .then((questionConfig) => set({ questionConfig }))
        .catch((err) => fail("读取问题识别配置", err));
    },

    saveQuestionConfig: (config) => {
      return ipc<QuestionConfig>("save_question_config", { config })
        .then((questionConfig) => set({ questionConfig }))
        .catch((err) => fail("保存问题识别配置", err));
    },

    resetQuestions: () =>
      set({ currentQuestion: null, questionManual: false, questions: [] }),
  };
});
