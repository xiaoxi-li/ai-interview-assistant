import { useEffect, useRef, useState } from "react";
import {
  AlertCircle,
  BookOpen,
  Check,
  Copy,
  ExternalLink,
  Loader2,
  RefreshCw,
  Sparkles,
} from "lucide-react";
import { cn } from "../../lib/utils";
import { useAnswerEvents } from "../../hooks/useAnswerEvents";
import { useT } from "../../hooks/useT";
import { useAppStore } from "../../stores/appStore";
import { useInterviewStore } from "../../stores/interviewStore";
import { useKnowledgeStore } from "../../stores/knowledgeStore";

/** 引用来源展示项（实时召回含摘要与得分，落库兜底只有标题） */
interface Citation {
  noteId: string;
  title: string;
  snippet: string;
  score: number | null;
}

export default function AnswerPanel() {
  const asrStatus = useInterviewStore((s) => s.asrStatus);
  const currentQaId = useInterviewStore((s) => s.currentQaId);
  const questionConfig = useInterviewStore((s) => s.questionConfig);
  const answerText = useInterviewStore((s) => s.answerText);
  const answerStatus = useInterviewStore((s) => s.answerStatus);
  const answerError = useInterviewStore((s) => s.answerError);
  const answerModel = useInterviewStore((s) => s.answerModel);
  const sources = useInterviewStore((s) => s.sources);
  const persistedSources = useInterviewStore((s) => s.persistedSources);
  const generateAnswer = useInterviewStore((s) => s.generateAnswer);
  const applyAnswerPush = useInterviewStore((s) => s.applyAnswerPush);
  const loadQuestionConfig = useInterviewStore((s) => s.loadQuestionConfig);
  const recallSources = useInterviewStore((s) => s.recallSources);

  const [copied, setCopied] = useState(false);
  const autoHandledRef = useRef<string | null>(null);
  const t = useT();

  const sessionId = asrStatus?.sessionId ?? null;
  const streaming = answerStatus === "streaming" || answerStatus === "thinking";

  useAnswerEvents(applyAnswerPush);

  useEffect(() => {
    loadQuestionConfig();
  }, [loadQuestionConfig]);

  // 识别到新问题后自动召回知识库片段（IA-07）：结果供下方「引用来源」展示
  useEffect(() => {
    if (sessionId && currentQaId) {
      recallSources(sessionId, currentQaId);
    }
  }, [sessionId, currentQaId, recallSources]);

  // 识别到新问题后自动生成回答（可在设置中关闭）
  useEffect(() => {
    if (!sessionId || !currentQaId) return;
    if (autoHandledRef.current === currentQaId) return;
    if (questionConfig && questionConfig.autoAnswer === false) return;

    autoHandledRef.current = currentQaId;
    generateAnswer(sessionId, currentQaId);
  }, [sessionId, currentQaId, questionConfig, generateAnswer]);

  const handleCopy = () => {
    if (!answerText) return;
    navigator.clipboard
      ?.writeText(answerText)
      .then(() => {
        setCopied(true);
        window.setTimeout(() => setCopied(false), 1500);
      })
      .catch((err) => console.error("复制回答失败:", err));
  };

  /** 在知识库中打开被引用的笔记（IA-09） */
  const openNote = (noteId: string) => {
    if (!noteId) return;
    useKnowledgeStore.getState().select(noteId);
    useAppStore.getState().setCurrentPage("knowledge");
  };

  // 优先使用实时召回结果（含摘要与得分）；无常驻结果时用落库引用兜底
  const citations: Citation[] = sources.length
    ? sources.map((hit) => ({
        noteId: hit.noteId,
        title: hit.title,
        snippet: hit.snippet,
        score: hit.score,
      }))
    : persistedSources.map((ref) => ({
        noteId: ref.noteId,
        title: ref.title,
        snippet: "",
        score: null,
      }));

  return (
    <section className="glass flex flex-col rounded-xl p-4 shadow-card">
      <header className="mb-3 flex items-center gap-2">
        <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg bg-gradient-to-br from-primary to-primary-3 shadow-brand">
          <Sparkles className="h-3.5 w-3.5 text-white" />
        </div>
        <h2 className="text-[13px] font-semibold">{t("AI 回答")}</h2>

        {answerModel && (
          <span className="rounded-full bg-ghost/[0.06] px-2 py-0.5 text-[10px] text-text-subtle">
            {answerModel}
          </span>
        )}
        {streaming && <Loader2 className="h-3.5 w-3.5 animate-spin text-primary-3" />}
        {answerStatus === "done" && (
          <span className="rounded-full bg-success/15 px-2 py-0.5 text-[10px] text-success">
            {t("已完成")}
          </span>
        )}

        <div className="ml-auto flex items-center gap-1">
          <button
            onClick={handleCopy}
            disabled={!answerText}
            title={t("复制回答")}
            className="focus-ring cursor-pointer rounded p-1 text-text-subtle transition-colors hover:bg-ghost/[0.06] hover:text-text disabled:cursor-not-allowed disabled:opacity-40"
          >
            {copied ? <Check className="h-3.5 w-3.5 text-success" /> : <Copy className="h-3.5 w-3.5" />}
          </button>
          <button
            onClick={() => sessionId && generateAnswer(sessionId, currentQaId, true)}
            disabled={!currentQaId || streaming}
            title={t("重新生成")}
            className="focus-ring flex cursor-pointer items-center gap-1 rounded-lg border border-ghost/10 px-2 py-1 text-[10px] text-text-muted transition-colors hover:border-primary/30 hover:text-primary-3 disabled:cursor-not-allowed disabled:opacity-40"
          >
            <RefreshCw className="h-3 w-3" />
            {t("重新生成")}
          </button>
        </div>
      </header>

      {answerError ? (
        <p className="flex items-start gap-1.5 rounded-md border border-danger/30 bg-danger/10 px-3 py-2 text-[11px] leading-relaxed text-danger">
          <AlertCircle className="mt-0.5 h-3 w-3 shrink-0" />
          <span>{answerError}</span>
        </p>
      ) : answerText ? (
        <p className="whitespace-pre-wrap text-[12.5px] leading-relaxed text-text">
          {answerText}
          {answerStatus === "streaming" && (
            <span className="ml-0.5 inline-block h-3.5 w-1.5 animate-pulse bg-primary-3 align-middle" />
          )}
        </p>
      ) : (
        <p className="rounded-md border border-ghost/[0.06] bg-background-3/30 px-3 py-6 text-center text-[11px] leading-relaxed text-text-subtle">
          {currentQaId
            ? streaming
              ? t("正在结合知识库生成回答…")
              : t("识别到问题后会自动生成回答，也可点击「重新生成」。")
            : t("尚未识别到问题。开始转写并识别到问题后，这里会输出可直接口述的回答。")}
        </p>
      )}

      {citations.length > 0 && (
        <div className="mt-3">
          <div className="mb-1.5 flex items-center gap-1.5">
            <BookOpen className="h-3.5 w-3.5 text-text-subtle" />
            <span className="text-[11px] font-medium text-text-muted">{t("引用来源")}</span>
            <span className="ml-auto text-[10px] text-text-subtle">
              {t("{count} 篇", { count: citations.length })}
            </span>
          </div>

          <ul className="flex flex-col gap-1.5">
            {citations.map((citation, index) => (
              <li key={`${citation.noteId || citation.title}-${index}`}>
                <button
                  type="button"
                  disabled={!citation.noteId}
                  onClick={() => openNote(citation.noteId)}
                  title={
                    citation.noteId
                      ? t("在知识库中打开该笔记")
                      : t("该引用缺少笔记 ID")
                  }
                  className={cn(
                    "focus-ring flex w-full flex-col gap-0.5 rounded-md border border-ghost/[0.06] bg-background-3/30 px-2.5 py-2 text-left transition-colors",
                    citation.noteId
                      ? "cursor-pointer hover:border-primary/30 hover:bg-primary/[0.07]"
                      : "cursor-default"
                  )}
                >
                  <span className="flex items-center gap-2">
                    <span className="text-[10px] text-text-subtle">[{index + 1}]</span>
                    <span className="truncate text-[11px] font-medium text-text">
                      {citation.title || t("未命名笔记")}
                    </span>
                    {citation.score !== null && (
                      <span className="ml-auto shrink-0 text-[10px] text-text-subtle">
                        {citation.score.toFixed(3)}
                      </span>
                    )}
                    {citation.noteId && (
                      <ExternalLink className="h-3 w-3 shrink-0 text-text-subtle" />
                    )}
                  </span>
                  {citation.snippet && (
                    <span className="line-clamp-2 text-[11px] leading-relaxed text-text-muted">
                      {citation.snippet}
                    </span>
                  )}
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}
