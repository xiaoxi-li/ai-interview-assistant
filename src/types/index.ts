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

export interface TranscriptSegment {
  id: string;
  speaker: Speaker;
  text: string;
  isFinal: boolean;
  timestamp: number;
}

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

export interface InterfaceConfig {
  theme: "dark" | "light";
  hotkeys: Record<string, string>;
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
