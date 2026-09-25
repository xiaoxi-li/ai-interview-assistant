import { useEffect, useRef, useState } from "react";
import { useKnowledgeStore } from "../stores/knowledgeStore";
import type { Note } from "../types";

/** 自动保存防抖间隔（毫秒） */
export const AUTOSAVE_DELAY = 800;

/**
 * 管理单篇笔记的编辑草稿，并在停止输入后自动保存。
 * 切换笔记时草稿会重置为最新持久化内容。
 */
export function useNoteDraft(note: Note | null) {
  const updateNote = useKnowledgeStore((s) => s.updateNote);
  const [title, setTitle] = useState(note?.title ?? "");
  const [content, setContent] = useState(note?.content ?? "");
  const [dirty, setDirty] = useState(false);
  const timerRef = useRef<number | null>(null);
  const noteId = note?.id ?? null;

  useEffect(() => {
    setTitle(note?.title ?? "");
    setContent(note?.content ?? "");
    setDirty(false);
  }, [noteId]);

  useEffect(() => {
    if (!dirty || !noteId) return;
    if (timerRef.current !== null) window.clearTimeout(timerRef.current);
    timerRef.current = window.setTimeout(() => {
      updateNote(noteId, title, content);
      setDirty(false);
    }, AUTOSAVE_DELAY);

    return () => {
      if (timerRef.current !== null) window.clearTimeout(timerRef.current);
    };
  }, [dirty, title, content, noteId, updateNote]);

  return {
    title,
    content,
    dirty,
    setTitle: (value: string) => {
      setTitle(value);
      setDirty(true);
    },
    setContent: (value: string) => {
      setContent(value);
      setDirty(true);
    },
  };
}
