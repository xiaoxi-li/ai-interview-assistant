import { useEffect, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { isTauri } from "../lib/utils";

/**
 * 监听窗口级文件拖拽。
 * 返回 `isDragging` 用于展示拖拽态提示；`onDrop` 通过 ref 保持稳定，避免重复订阅。
 */
export function useFileDrop(onDrop: (paths: string[]) => void) {
  const [isDragging, setIsDragging] = useState(false);
  const handlerRef = useRef(onDrop);

  useEffect(() => {
    handlerRef.current = onDrop;
  }, [onDrop]);

  useEffect(() => {
    if (!isTauri()) return;

    const webview = getCurrentWebview();
    const unlisten = webview.onDragDropEvent((event) => {
      const payload = event.payload;
      if (payload.type === "enter" || payload.type === "over") {
        setIsDragging(true);
      } else if (payload.type === "drop") {
        setIsDragging(false);
        if (payload.paths.length > 0) {
          handlerRef.current(payload.paths);
        }
      } else {
        setIsDragging(false);
      }
    });

    return () => {
      unlisten
        .then((fn) => fn())
        .catch((err) => console.error("取消拖拽监听失败:", err));
    };
  }, []);

  return { isDragging };
}
