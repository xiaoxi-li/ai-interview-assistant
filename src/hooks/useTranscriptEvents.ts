import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { isTauri } from "../lib/utils";
import type { TranscriptPush } from "../types";

/**
 * 订阅本地 ASR 转写事件（`asr://transcript`）。
 * 回调经 ref 保持稳定，避免重渲染时反复订阅。
 */
export function useTranscriptEvents(onPush: (push: TranscriptPush) => void) {
  const handlerRef = useRef(onPush);

  useEffect(() => {
    handlerRef.current = onPush;
  }, [onPush]);

  useEffect(() => {
    if (!isTauri()) return;

    const unlisten = listen<TranscriptPush>("asr://transcript", (event) => {
      handlerRef.current(event.payload);
    });

    return () => {
      unlisten
        .then((fn) => fn())
        .catch((err) => console.error("取消转写事件监听失败:", err));
    };
  }, []);
}
