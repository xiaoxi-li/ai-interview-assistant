import { useEffect, useRef, useState } from "react";
import { AlertCircle, Check, Copy, Loader2, RefreshCw, Sparkles } from "lucide-react";
import { cn } from "../../lib/utils";
import { useAnswerEvents } from "../../hooks/useAnswerEvents";
import { useInterviewStore } from "../../stores/interviewStore";

export default function AnswerPanel() {
  const asrStatus = useInterviewStore((s) => s.asrStatus);
  const currentQaId = useInterviewStore((s) => s.currentQaId);
  const questionConfig = useInterviewStore((s) => s.questionConfig);
  const answerText = useInterviewStore((s) => s.answerText);
  const answerStatus = useInterviewStore((s) => s.answerStatus);
  const answerError = useInterviewStore((s) => s.answerError);
  const answerModel = useInterviewStore((s) => s.answerModel);
  const sources = useInterviewStore((s) => s.sources);
  const generateAnswer = useInterviewStore((s) => s.generateAnswer);
  const applyAnswerPush = useInterviewStore((s) => s.applyAnswerPush);
  const loadQuestionConfig = useInterviewStore((s) => s.loadQuestionConfig);

  const [copied, setCopied] = useState(false);
  const autoHandledRef = useRef<string | null>(null);

  const sessionId = asrStatus?.sessionId ?? null;
  const streaming = answerStatus === "streaming" || answerStatus === "thinking";

  useAnswerEvents(applyAnswerPush);

  useEffect(() => {
    loadQuestionConfig();
  }, [loadQuestionConfig]);

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

  return (
    <section className="glass flex flex-col rounded-xl p-4">
      <header className="mb-3 flex items-center gap-2">
        <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg bg-gradient-to-br from-primary to-primary-3 shadow-lg shadow-primary/25">
          <Sparkles className="h-3.5 w-3.5 text-white" />
        </div>
        <h2 className="text-[13px] font-semibold">AI 回答</h2>

        {answerModel && (
          <span className="rounded-full bg-white/[0.06] px-2 py-0.5 text-[10px] text-text-subtle">
            {answerModel}
          </span>
        )}
        {streaming && <Loader2 className="h-3.5 w-3.5 animate-spin text-primary-3" />}
        {answerStatus === "done" && (
          <span className="rounded-full bg-success/15 px-2 py-0.5 text-[10px] text-success">
            已完成
          </span>
        )}

        <div className="ml-auto flex items-center gap-1">
          <button
            onClick={handleCopy}
            disabled={!answerText}
            title="复制回答"
            className="cursor-pointer rounded p-1 text-text-subtle transition-colors hover:bg-white/10 hover:text-text disabled:cursor-not-allowed disabled:opacity-40"
          >
            {copied ? <Check className="h-3.5 w-3.5 text-success" /> : <Copy className="h-3.5 w-3.5" />}
          </button>
          <button
            onClick={() => sessionId && generateAnswer(sessionId, currentQaId, true)}
            disabled={!currentQaId || streaming}
            title="重新生成回答"
            className="flex cursor-pointer items-center gap-1 rounded border border-white/10 px-2 py-1 text-[10px] text-text-muted transition-colors hover:border-primary/30 hover:text-primary-3 disabled:cursor-not-allowed disabled:opacity-40"
          >
            <RefreshCw className="h-3 w-3" />
            重新生成
          </button>
        </div>
      </header>

      {answerError ? (
        <p className="flex items-start gap-1.5 rounded-lg border border-danger/30 bg-danger/10 px-3 py-2 text-[11px] leading-relaxed text-danger">
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
        <p className="rounded-lg border border-white/[0.06] bg-background-3/30 px-3 py-6 text-center text-[11px] leading-relaxed text-text-subtle">
          {currentQaId
            ? streaming
              ? "正在结合知识库生成回答…"
              : "识别到问题后会自动生成回答，也可点击「重新生成」。"
            : "尚未识别到问题。开始转写并识别到问题后，这里会输出可直接口述的回答。"}
        </p>
      )}

      {sources.length > 0 && (
        <div className="mt-2 flex flex-wrap items-center gap-1.5">
          <span className="text-[10px] text-text-subtle">引用：</span>
          {sources.map((hit) => (
            <span
              key={hit.noteId}
              className={cn(
                "max-w-[10rem] truncate rounded bg-white/[0.06] px-1.5 py-0.5 text-[10px] text-text-muted"
              )}
              title={hit.title}
            >
              {hit.title}
            </span>
          ))}
        </div>
      )}
    </section>
  );
}
