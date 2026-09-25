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
  apiKeyRef: string;
  model: string;
  stream: boolean;
  temperature: number;
}

export type ASRMode = "cloud" | "local";

export interface ASRConfig {
  mode: ASRMode;
  model: string;
  language: string;
  vadThreshold: number;
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
  isDefault: boolean;
}

export interface ConnectionTestResult {
  ok: boolean;
  latencyMs: number;
  message: string;
}

export type ViewMode = "source" | "preview" | "split";
