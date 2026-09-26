import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { isTauri } from "../lib/utils";
import type { ModelProgress } from "../types";

/**
 * 订阅模型下载进度事件（`model://progress`）。
 * 回调通过 ref 保持稳定，避免因组件重渲染反复订阅。
 */
export function useModelProgress(onProgress: (progress: ModelProgress) => void) {
  const handlerRef = useRef(onProgress);

  useEffect(() => {
    handlerRef.current = onProgress;
  }, [onProgress]);

  useEffect(() => {
    if (!isTauri()) return;

    const unlisten = listen<ModelProgress>("model://progress", (event) => {
      handlerRef.current(event.payload);
    });

    return () => {
      unlisten
        .then((fn) => fn())
        .catch((err) => console.error("取消模型进度监听失败:", err));
    };
  }, []);
}
