import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { isTauri } from "../lib/utils";
import type { AnswerPush } from "../types";

/**
 * 订阅 AI 回答流事件（`interview://answer`）。
 */
export function useAnswerEvents(onPush: (push: AnswerPush) => void) {
  const handlerRef = useRef(onPush);

  useEffect(() => {
    handlerRef.current = onPush;
  }, [onPush]);

  useEffect(() => {
    if (!isTauri()) return;

    const unlisten = listen<AnswerPush>("interview://answer", (event) => {
      handlerRef.current(event.payload);
    });

    return () => {
      unlisten
        .then((fn) => fn())
        .catch((err) => console.error("取消回答事件监听失败:", err));
    };
  }, []);
}
