import { create } from "zustand";
import { ipc } from "../lib/ipc";
import type {
  AnswerAck,
  AnswerPush,
  AnswerStatus,
  AsrEngineInfo,
  AsrSessionStatus,
  AudioDevice,
  CaptureStatus,
  DetectedQuestion,
  QaPair,
  QuestionConfig,
  QuestionPush,
  RecallHit,
  RecallOutcome,
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

/** 面试默认召回条数（与后端 DEFAULT_TOP_K 保持一致） */
export const DEFAULT_RECALL_TOP_K = 3;

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
  /** 当前问题对应的问答记录 ID（召回/回答的落库锚点） */
  currentQaId: string | null;
  /** 当前问题的知识库召回片段（IA-07） */
  sources: RecallHit[];
  sourcesLoading: boolean;
  /** 最近一次召回使用的查询（便于排查召回质量） */
  recallQueries: string[];

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
  /** 暂停 / 继续采集（IA-12） */
  setAsrPaused: (paused: boolean) => Promise<void>;
  loadQuestions: (sessionId: string) => Promise<void>;
  loadQuestionConfig: () => Promise<void>;
  saveQuestionConfig: (config: QuestionConfig) => Promise<void>;
  resetQuestions: () => void;

  recallSources: (sessionId: string, questionId?: string | null) => Promise<void>;
  resetSources: () => void;

  /** AI 回答（IA-08） */
  answerText: string;
  answerStatus: AnswerStatus;
  answerError: string | null;
  answerQaId: string | null;
  answerModel: string | null;
  generateAnswer: (
    sessionId: string,
    questionId?: string | null,
    regenerate?: boolean
  ) => Promise<void>;
  applyAnswerPush: (push: AnswerPush) => void;
  resetAnswer: () => void;

  /** 从落库记录恢复当前问题与回答（迷你浮窗 / 重开后使用） */
  hydrateFromPair: (pair: QaPair) => void;
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
    currentQaId: null,
    sources: [],
    sourcesLoading: false,
    recallQueries: [],

    answerText: "",
    answerStatus: "idle",
    answerError: null,
    answerQaId: null,
    answerModel: null,

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
            currentQaId: null,
            sources: [],
            recallQueries: [],
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
        // 新问题出现即清空上一题的召回结果与回答
        currentQaId: qaId,
        sources: [],
        recallQueries: [],
        answerText: "",
        answerStatus: "idle",
        answerError: null,
        answerQaId: null,
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

    setAsrPaused: (paused) => {
      return ipc<AsrSessionStatus>("set_asr_paused", { paused })
        .then((asrStatus) => set({ asrStatus }))
        .catch((err) => {
          console.error("切换采集暂停失败:", err);
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
      set({
        currentQuestion: null,
        questionManual: false,
        questions: [],
        currentQaId: null,
        sources: [],
        recallQueries: [],
      }),

    recallSources: (sessionId, questionId) => {
      set({ sourcesLoading: true });
      return ipc<RecallOutcome>("recall_for_question", {
        sessionId,
        questionId: questionId ?? null,
        topK: DEFAULT_RECALL_TOP_K,
      })
        .then((outcome) =>
          set({
            sources: outcome.hits,
            recallQueries: outcome.queries,
            sourcesLoading: false,
          })
        )
        .catch((err) => {
          console.error("知识库召回失败:", err);
          set({ sourcesLoading: false, sources: [], recallQueries: [] });
        });
    },

    resetSources: () => set({ sources: [], recallQueries: [] }),

    generateAnswer: (sessionId, questionId, regenerate) => {
      set({ answerStatus: "thinking", answerError: null, answerText: "" });
      return ipc<AnswerAck>("generate_answer", {
        sessionId,
        questionId: questionId ?? null,
        regenerate: regenerate ?? false,
      })
        .then((ack) => {
          if (!ack.reused) {
            set({ answerQaId: ack.qaId });
            return;
          }
          // 已有回答：直接取回，不重复消耗额度
          return ipc<QaPair>("get_question", { id: ack.qaId }).then((pair) =>
            set({
              answerQaId: pair.id,
              answerText: pair.answer,
              answerStatus: "done",
            })
          );
        })
        .catch((err) => {
          console.error("生成回答失败:", err);
          set({ answerStatus: "error", answerError: String(err) });
        });
    },

    applyAnswerPush: (push) => {
      const { kind, qaId, text, answer, message, model } = push;

      if (kind === "start") {
        set({
          answerQaId: qaId,
          answerText: "",
          answerStatus: "streaming",
          answerError: null,
          answerModel: model ?? null,
        });
        return;
      }

      if (kind === "token") {
        set((state) => ({ answerText: state.answerText + (text ?? "") }));
        return;
      }

      if (kind === "done") {
        set((state) => ({
          answerQaId: qaId,
          answerText: answer ?? state.answerText,
          answerStatus: "done",
          answerError: null,
        }));
        return;
      }

      set((state) => ({
        answerQaId: qaId,
        answerText: answer ?? state.answerText,
        answerStatus: "error",
        answerError: message ?? "生成回答失败",
      }));
    },

    resetAnswer: () =>
      set({
        answerText: "",
        answerStatus: "idle",
        answerError: null,
        answerQaId: null,
        answerModel: null,
      }),

    hydrateFromPair: (pair) =>
      set({
        currentQaId: pair.id,
        currentQuestion: {
          text: pair.question,
          score: 0,
          signals: [],
          ts: pair.ts,
        },
        answerQaId: pair.answer ? pair.id : null,
        answerText: pair.answer,
        answerStatus: pair.answer ? "done" : "idle",
        answerError: null,
      }),
  };
});
