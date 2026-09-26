export interface Note {
  id: string;
  title: string;
  content: string;
  groupId: string | null;
  tags: string[];
  createdAt: number;
  updatedAt: number;
  isDeleted: boolean;
}

export interface Group {
  id: string;
  name: string;
  parentId: string | null;
  order: number;
}

/** 笔记历史版本 */
export interface NoteVersion {
  id: string;
  noteId: string;
  title: string;
  content: string;
  /** 递增版本号（越大越新） */
  version: number;
  createdAt: number;
}

export interface Tag {
  id: string;
  name: string;
  color: string;
}

export interface SearchResult {
  noteId: string;
  title: string;
  snippet: string;
  score: number;
  matchedChunk?: string;
}

/** 全文检索命中（与后端 SearchHit 对应） */
export interface SearchHit {
  noteId: string;
  title: string;
  snippet: string;
  score: number;
}

export type Speaker = "interviewer" | "me";

export type AnswerStatus = "idle" | "thinking" | "streaming" | "done" | "error";

export interface AnswerState {
  question: string;
  text: string;
  sources: SearchResult[];
  status: AnswerStatus;
}

export interface QAPair {
  id: string;
  question: string;
  answer: string;
  sources: string[];
  timestamp: number;
}

export interface Session {
  id: string;
  title: string;
  startedAt: number;
  endedAt?: number;
  segments: TranscriptSegment[];
  qaList: QAPair[];
}

export type AIPlatform = "deepseek" | "qwen" | "glm" | "chatgpt";

export interface AIConfig {
  platform: AIPlatform;
  baseUrl: string;
  model: string;
  stream: boolean;
  temperature: number;
}

/** 本地 ASR 转写事件（与后端 TranscriptEvent 对应） */
export type TranscriptEvent =
  | { kind: "partial"; speaker: Speaker; text: string }
  | { kind: "final"; speaker: Speaker; text: string }
  | { kind: "error"; message: string };

/** 后端推送的转写事件载荷（`asr://transcript`） */
export interface TranscriptPush {
  sessionId: string;
  event: TranscriptEvent;
  ts: number;
}

/** 本地 ASR 会话状态 */
export interface AsrSessionStatus {
  running: boolean;
  sessionId: string | null;
  source: "mic" | "system" | null;
  speaker: Speaker | null;
  engine: string | null;
  startedAt: number | null;
}

/** 本地 ASR 引擎信息 */
export interface AsrEngineInfo {
  id: string;
  label: string;
  ready: boolean;
  detail: string;
}

/** 转写段落（落库记录，与后端 TranscriptSegmentDto 对应） */
export interface TranscriptSegment {
  id: string;
  sessionId: string;
  speaker: Speaker;
  text: string;
  isFinal: boolean;
  ts: number;
}

/** 识别出的面试官问题 */
export interface DetectedQuestion {
  text: string;
  score: number;
  /** 命中的可读信号（如「含疑问词「怎么」」） */
  signals: string[];
  ts: number;
}

/** 后端推送的问题事件（`interview://question`） */
export interface QuestionPush {
  sessionId: string;
  /** 是否为手动触发（快捷键/按钮） */
  manual: boolean;
  question: DetectedQuestion;
  /** 落库后的问答记录 ID */
  qaId: string | null;
}

/** 问答对（qa_pairs） */
export interface QaPair {
  id: string;
  sessionId: string;
  question: string;
  answer: string;
  sources: string[];
  ts: number;
}

/** 知识库召回片段（IA-07） */
export interface RecallHit {
  noteId: string;
  title: string;
  snippet: string;
  score: number;
}

/** 一次召回的过程与结果 */
export interface RecallOutcome {
  question: string;
  /** 实际使用的检索查询（完整问句 + 关键词问句） */
  queries: string[];
  hits: RecallHit[];
}

/** 问题识别配置 */
export interface QuestionConfig {
  autoDetect: boolean;
  threshold: number;
  minChars: number;
  mergeWindowMs: number;
}

/** 单段文本的问题判定结果（预览/调试） */
export interface QuestionScore {
  isQuestion: boolean;
  score: number;
  signals: string[];
}

/** 音频采集状态 */
export interface CaptureStatus {
  running: boolean;
  source: "mic" | "system" | null;
  micDevice: string;
  systemDevice: string;
  loopbackAvailable: boolean;
}

/** 后端推送的音频电平事件 */
export interface AudioLevelEvent {
  source: string;
  level: number;
  ts: number;
  frames: number;
}

/** 知识库存储配置（路径变更需重启生效） */
export interface KnowledgeConfig {
  dbPath: string | null;
  modelsDir: string | null;
  embeddingModel: string;
}

/** 当前生效的存储路径 */
export interface StoragePaths {
  dbPath: string;
  modelsDir: string;
}

export type ModelKind = "embedding" | "vad" | "asr";

/** 本地模型清单条目 */
export interface ModelSpec {
  id: string;
  name: string;
  kind: ModelKind;
  fileName: string;
  path: string;
  sha256: string;
  sizeBytes: number;
  description: string;
  requiredForLocalAsr: boolean;
}

/** 模型安装状态 */
export interface ModelStatus extends ModelSpec {
  installed: boolean;
  resolvedPath: string | null;
  partialBytes: number;
}

/** 下载进度事件载荷 */
export interface ModelProgress {
  id: string;
  downloaded: number;
  total: number | null;
}

/** 连接测试结果 */
export interface ConnectionTest {
  ok: boolean;
  latencyMs: number;
  message: string;
}

/** 平台预设（Base URL 与默认模型） */
export interface PlatformPreset {
  platform: AIPlatform;
  label: string;
  defaultBaseUrl: string;
  defaultModel: string;
}

export type ASRMode = "cloud" | "local";
export type CloudAsrProvider = "xfyun" | "aliyun" | "tencent";

export interface ASRConfig {
  mode: ASRMode;
  cloudProvider: CloudAsrProvider;
  /** 云端模型名 */
  model: string;
  language: string;
  /** VAD 阈值 0 ~ 1 */
  vadThreshold: number;
  /** 本地模型名（sherpa-onnx） */
  localModel: string;
}

/** 云端 ASR 服务商预设 */
export interface AsrProviderPreset {
  provider: CloudAsrProvider;
  label: string;
  defaultModel: string;
  credentialFields: string[];
}

/** 单个凭证字段状态 */
export interface CredentialFieldStatus {
  field: string;
  configured: boolean;
  masked: string;
}

/** 服务商凭证整体状态 */
export interface AsrCredentialStatus {
  provider: string;
  complete: boolean;
  fields: CredentialFieldStatus[];
}

export interface AudioConfig {
  micDeviceId: string | null;
  systemDeviceId: string | null;
}

export type HotkeyAction =
  | "ask"
  | "regenerate"
  | "copy_answer"
  | "pause_capture"
  | "toggle_mini";

export interface HotkeyBinding {
  action: string;
  accelerator: string;
}

export interface InterfaceConfig {
  theme: "dark" | "light";
  hotkeys: HotkeyBinding[];
  autoStart: boolean;
  trayBehavior: "minimize" | "quit";
}

export interface PrivacyConfig {
  uploadAudio: boolean;
  encryptLocal: boolean;
}

export interface AppSettings {
  ai: AIConfig;
  asr: ASRConfig;
  audio: AudioConfig;
  interface: InterfaceConfig;
  privacy: PrivacyConfig;
}

export interface AudioDevice {
  id: string;
  name: string;
  /** input = 麦克风；output = 系统音频来源 */
  kind: "input" | "output";
  isDefault: boolean;
}

export interface ConnectionTestResult {
  ok: boolean;
  latencyMs: number;
  message: string;
}

export type ViewMode = "source" | "preview" | "split";
