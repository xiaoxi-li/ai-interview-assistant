import { useEffect, useRef } from "react";
import { useInterviewStore } from "../stores/interviewStore";

/**
 * 面试辅助链路的「自动召回 + 自动生成回答」唯一触发实现（IA-07 / IA-08 / IA-13）。
 *
 * 由**答案展示方**调用：全屏 `AnswerPanel` 与迷你页 `MiniView`。
 * - 迷你页此前被当作纯展示端（只订阅事件、不触发生成），导致迷你模式下回答不流式；
 *   触发逻辑收敛到本 hook 后，两处消费者行为完全一致。
 * - 两个消费者互斥挂载（`App.tsx` 迷你模式早退渲染 `MiniView`），不存在重复触发。
 * - `sources` 的唯一写入方仍是 `recallSources`，本 hook 不写 `sources`（触发点也不得放回 `QuestionPanel`，
 *   否则「引用来源」会退化为仅标题的 `persistedSources` 兜底）。
 */
export function useAnswerPipeline(): void {
  const sessionId = useInterviewStore((s) => s.asrStatus?.sessionId ?? null);
  const currentQaId = useInterviewStore((s) => s.currentQaId);
  const questionConfig = useInterviewStore((s) => s.questionConfig);
  const loadQuestionConfig = useInterviewStore((s) => s.loadQuestionConfig);
  const recallSources = useInterviewStore((s) => s.recallSources);
  const generateAnswer = useInterviewStore((s) => s.generateAnswer);

  // 按挂载期去重：同一问题只触发一次生成（切页/进出迷你模式时行为与改造前一致）
  const autoHandledRef = useRef<string | null>(null);

  useEffect(() => {
    loadQuestionConfig();
  }, [loadQuestionConfig]);

  // 识别到新问题后自动召回知识库片段（IA-07）：结果供「引用来源」展示
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
}
