import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { isTauri } from "../lib/utils";
import type { QuestionPush } from "../types";

/**
 * 订阅问题识别事件（`interview://question`）。
 */
export function useQuestionEvents(onPush: (push: QuestionPush) => void) {
  const handlerRef = useRef(onPush);

  useEffect(() => {
    handlerRef.current = onPush;
  }, [onPush]);

  useEffect(() => {
    if (!isTauri()) return;

    const unlisten = listen<QuestionPush>("interview://question", (event) => {
      handlerRef.current(event.payload);
    });

    return () => {
      unlisten
        .then((fn) => fn())
        .catch((err) => console.error("取消问题事件监听失败:", err));
    };
  }, []);
}
