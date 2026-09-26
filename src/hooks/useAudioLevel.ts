import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { isTauri } from "../lib/utils";
import type { AudioLevelEvent } from "../types";

/**
 * 订阅后端推送的音频电平事件（`audio://level`）。
 * 原生事件只在 Tauri 环境存在，浏览器调试时静默跳过。
 */
export function useAudioLevel(onLevel: (level: number) => void) {
  const handlerRef = useRef(onLevel);

  useEffect(() => {
    handlerRef.current = onLevel;
  }, [onLevel]);

  useEffect(() => {
    if (!isTauri()) return;

    const unlisten = listen<AudioLevelEvent>("audio://level", (event) => {
      handlerRef.current(event.payload.level);
    });

    return () => {
      unlisten
        .then((fn) => fn())
        .catch((err) => console.error("取消音频电平监听失败:", err));
    };
  }, []);
}
