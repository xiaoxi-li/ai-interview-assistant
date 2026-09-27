import { useCallback, useEffect } from "react";
import { BookOpen, HelpCircle, Loader2, RefreshCw } from "lucide-react";
import { cn } from "../../lib/utils";
import { useQuestionEvents } from "../../hooks/useQuestionEvents";
import { useT } from "../../hooks/useT";
import { useInterviewStore } from "../../stores/interviewStore";

/** 分数 → 展示语气 */
function scoreTone(score: number): string {
  if (score >= 1.0) return "bg-success/15 text-success";
  if (score >= 0.6) return "bg-primary/15 text-primary-3";
  return "bg-warning/15 text-warning";
}

export default function QuestionPanel() {
  const asrStatus = useInterviewStore((s) => s.asrStatus);
  const currentQuestion = useInterviewStore((s) => s.currentQuestion);
  const questionManual = useInterviewStore((s) => s.questionManual);
  const questions = useInterviewStore((s) => s.questions);
  const currentQaId = useInterviewStore((s) => s.currentQaId);
  const sources = useInterviewStore((s) => s.sources);
  const sourcesLoading = useInterviewStore((s) => s.sourcesLoading);
  const recallQueries = useInterviewStore((s) => s.recallQueries);
  const applyQuestionPush = useInterviewStore((s) => s.applyQuestionPush);
  const triggerQuestion = useInterviewStore((s) => s.triggerQuestion);
  const loadQuestions = useInterviewStore((s) => s.loadQuestions);
  const recallSources = useInterviewStore((s) => s.recallSources);

  const t = useT();
  const sessionId = asrStatus?.sessionId ?? null;

  useQuestionEvents(applyQuestionPush);

  const refresh = useCallback(() => {
    if (sessionId) loadQuestions(sessionId);
  }, [sessionId, loadQuestions]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  // 识别到新问题后自动召回知识库片段（IA-07）
  useEffect(() => {
    if (sessionId && currentQaId) {
      recallSources(sessionId, currentQaId);
    }
  }, [sessionId, currentQaId, recallSources]);

  return (
    <section className="glass flex flex-col rounded-xl p-4 shadow-card">
      <header className="mb-3 flex items-center gap-2">
        <HelpCircle className="h-4 w-4 text-primary-3" />
        <h2 className="text-[13px] font-semibold">{t("问题识别")}</h2>
        {currentQuestion && (
          <span
            className={cn(
              "rounded-full px-2 py-0.5 text-[10px]",
              questionManual ? "bg-warning/15 text-warning" : "bg-primary/15 text-primary-3"
            )}
          >
            {questionManual ? t("手动触发") : t("自动识别")}
          </span>
        )}
        <span className="ml-auto text-[10px] text-text-subtle">
          {t("已识别 {count} 个问题", { count: questions.length })}
        </span>
      </header>

      {currentQuestion ? (
        <>
          <div className="rounded-md border border-primary/25 bg-primary/[0.07] p-3">
            <p className="text-[13px] font-medium leading-relaxed text-text">
              {currentQuestion.text}
            </p>

            <div className="mt-2 flex flex-wrap items-center gap-1.5">
              <span
                className={cn(
                  "rounded px-1.5 py-0.5 text-[10px]",
                  scoreTone(currentQuestion.score)
                )}
                title={t("判定得分（阈值 0.5，越高越确定）")}
              >
                {t("置信度")} {currentQuestion.score.toFixed(2)}
              </span>
              {currentQuestion.signals.map((signal) => (
                <span
                  key={signal}
                  className="rounded bg-white/[0.06] px-1.5 py-0.5 text-[10px] text-text-muted"
                >
                  {signal}
                </span>
              ))}
            </div>
          </div>

          {/* 知识库召回片段 */}
          <div className="mt-3">
            <div className="mb-1.5 flex items-center gap-1.5">
              <BookOpen className="h-3.5 w-3.5 text-text-subtle" />
              <span className="text-[11px] font-medium text-text-muted">{t("知识库参考")}</span>
              {sourcesLoading && (
                <Loader2 className="h-3 w-3 animate-spin text-text-subtle" />
              )}
              {!sourcesLoading && (
                <span className="ml-auto text-[10px] text-text-subtle">
                  {t("{count} 条片段", { count: sources.length })}
                </span>
              )}
            </div>

            {sources.length > 0 ? (
              <ul className="flex flex-col gap-1.5">
                {sources.map((hit) => (
                  <li
                    key={hit.noteId}
                    className="rounded-md border border-white/[0.06] bg-background-3/30 px-2.5 py-2"
                  >
                    <div className="flex items-center gap-2">
                      <span className="truncate text-[11px] font-medium text-text">
                        {hit.title}
                      </span>
                      <span className="ml-auto shrink-0 text-[10px] text-text-subtle">
                        {hit.score.toFixed(3)}
                      </span>
                    </div>
                    <p className="mt-1 line-clamp-2 text-[11px] leading-relaxed text-text-muted">
                      {hit.snippet}
                    </p>
                  </li>
                ))}
              </ul>
            ) : (
              !sourcesLoading && (
                <p className="rounded-md border border-white/[0.06] bg-background-3/30 px-2.5 py-3 text-[11px] leading-relaxed text-text-subtle">
                  {t("知识库中未召回到相关内容")}
                  {recallQueries.length > 0 &&
                    t("（查询：{queries}）", { queries: recallQueries.join(" / ") })}
                </p>
              )
            )}
          </div>
        </>
      ) : (
        <p className="rounded-md border border-white/[0.06] bg-background-3/30 px-3 py-6 text-center text-[11px] leading-relaxed text-text-subtle">
          {asrStatus?.running
            ? t("正在监听面试官语音，识别到问题后会显示在这里。")
            : t("开始转写后，面试官的问题会被自动识别；也可随时手动触发识别。")}
        </p>
      )}

      {questions.length > 1 && (
        <ul className="mt-3 flex max-h-32 flex-col gap-1 overflow-y-auto">
          {questions
            .slice(-4)
            .reverse()
            .map((pair) => (
              <li
                key={pair.id}
                className="truncate rounded-md border border-white/[0.06] bg-background-3/30 px-2 py-1 text-[11px] text-text-muted"
                title={pair.question}
              >
                {pair.question}
              </li>
            ))}
        </ul>
      )}

      <button
        onClick={() => triggerQuestion()}
        disabled={!asrStatus?.running}
        className="focus-ring mt-3 flex w-full cursor-pointer items-center justify-center gap-1.5 rounded-lg border border-white/10 bg-background-3/60 px-4 py-2 text-[12px] text-text-muted transition-colors hover:border-primary/30 hover:text-primary-3 disabled:cursor-not-allowed disabled:opacity-50"
        title={t("用最近的面试官语音立即识别一次问题")}
      >
        <RefreshCw className="h-3.5 w-3.5" />
        {t("手动识别当前问题")}
      </button>
    </section>
  );
}
