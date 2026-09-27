import { FileText, RotateCcw, Trash2, Undo2 } from "lucide-react";
import { cn, excerpt, formatRelative } from "../../lib/utils";
import { NOTE_DRAG_TYPE } from "../../lib/dnd";
import type { Note, SearchHit } from "../../types";

const ICON_BTN =
  "focus-ring flex h-6 w-6 shrink-0 cursor-pointer items-center justify-center rounded text-text-subtle transition-colors";

interface NoteListProps {
  notes: Note[];
  selectedId: string | null;
  keyword: string;
  mode: "all" | "trash";
  /** 服务端全文检索命中；提供时按命中顺序展示并显示摘要 */
  hits?: SearchHit[];
  onSelect: (id: string) => void;
  onTrash: (id: string) => void;
  onRestore: (id: string) => void;
  onPurge: (id: string) => void;
}

export default function NoteList({
  notes,
  selectedId,
  keyword,
  mode,
  hits,
  onSelect,
  onTrash,
  onRestore,
  onPurge,
}: NoteListProps) {
  const searching = Boolean(hits);
  const kw = keyword.trim().toLowerCase();
  // 服务端检索已排序过滤，此处不再重复过滤
  const filtered = searching
    ? notes
    : kw
      ? notes.filter(
          (n) =>
            n.title.toLowerCase().includes(kw) ||
            n.content.toLowerCase().includes(kw) ||
            n.tags.some((t) => t.toLowerCase().includes(kw))
        )
      : notes;

  const snippetOf = (note: Note): string => {
    const hit = hits?.find((h) => h.noteId === note.id);
    return hit?.snippet || excerpt(note.content, 70);
  };

  if (filtered.length === 0) {
    return (
      <p className="px-2 py-6 text-center text-[12px] leading-relaxed text-text-subtle">
        {notes.length === 0
          ? mode === "trash"
            ? "回收站为空"
            : "暂无笔记，导入或新建 Markdown 开始使用"
          : "没有匹配的笔记"}
      </p>
    );
  }

  return (
    <ul className="flex flex-col gap-1">
      {filtered.map((note) => {
        const active = mode === "all" && selectedId === note.id;
        return (
          <li key={note.id}>
            <div
              role="button"
              tabIndex={0}
              draggable={mode === "all"}
              onDragStart={(e) => {
                // 拖到群组树上即可批量归类
                e.dataTransfer.setData(NOTE_DRAG_TYPE, note.id);
              }}
              onClick={() => mode === "all" && onSelect(note.id)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && mode === "all") onSelect(note.id);
              }}
              className={cn(
                "focus-ring group w-full rounded-lg border px-3 py-2 text-left transition-all duration-150",
                mode === "all" ? "cursor-pointer" : "cursor-default opacity-90",
                active
                  ? "border-primary/40 bg-primary/12"
                  : "border-transparent hover:border-white/10 hover:bg-white/[0.06]"
              )}
            >
              <div className="flex items-center gap-2">
                <FileText
                  className={cn(
                    "h-3.5 w-3.5 shrink-0",
                    active ? "text-primary-3" : "text-text-subtle"
                  )}
                />
                <span className="min-w-0 flex-1 truncate text-[13px] font-medium text-text">
                  {note.title || "未命名笔记"}
                </span>

                {mode === "all" ? (
                  <button
                    title="移入回收站"
                    onClick={(e) => {
                      e.stopPropagation();
                      onTrash(note.id);
                    }}
                    className={cn(
                      ICON_BTN,
                      "opacity-0 group-hover:opacity-100 hover:bg-danger/20 hover:text-danger"
                    )}
                  >
                    <Trash2 className="h-3.5 w-3.5" />
                  </button>
                ) : (
                  <span className="flex items-center gap-1">
                    <button
                      title="恢复"
                      onClick={(e) => {
                        e.stopPropagation();
                        onRestore(note.id);
                      }}
                      className={cn(ICON_BTN, "hover:bg-success/20 hover:text-success")}
                    >
                      <Undo2 className="h-3.5 w-3.5" />
                    </button>
                    <button
                      title="彻底删除"
                      onClick={(e) => {
                        e.stopPropagation();
                        onPurge(note.id);
                      }}
                      className={cn(ICON_BTN, "hover:bg-danger/20 hover:text-danger")}
                    >
                      <RotateCcw className="h-3.5 w-3.5 rotate-45" />
                    </button>
                  </span>
                )}
              </div>

              <p className="mt-1 line-clamp-2 pl-[22px] text-[11px] leading-relaxed text-text-subtle">
                {snippetOf(note) || "（空白笔记）"}
              </p>
              <div className="mt-1 pl-[22px] text-[10px] text-text-subtle/70">
                {formatRelative(note.updatedAt)}
              </div>
            </div>
          </li>
        );
      })}
    </ul>
  );
}
