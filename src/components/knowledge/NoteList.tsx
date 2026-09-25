import { FileText } from "lucide-react";
import { cn, excerpt, formatRelative } from "../../lib/utils";
import type { Note } from "../../types";

interface NoteListProps {
  notes: Note[];
  selectedId: string | null;
  keyword: string;
  onSelect: (id: string) => void;
}

export default function NoteList({
  notes,
  selectedId,
  keyword,
  onSelect,
}: NoteListProps) {
  const kw = keyword.trim().toLowerCase();
  const filtered = kw
    ? notes.filter(
        (n) =>
          n.title.toLowerCase().includes(kw) ||
          n.content.toLowerCase().includes(kw) ||
          n.tags.some((t) => t.toLowerCase().includes(kw))
      )
    : notes;

  if (filtered.length === 0) {
    return (
      <p className="px-2 py-6 text-center text-[12px] leading-relaxed text-text-subtle">
        {notes.length === 0 ? "暂无笔记，导入 Markdown 开始使用" : "没有匹配的笔记"}
      </p>
    );
  }

  return (
    <ul className="flex flex-col gap-1">
      {filtered.map((note) => (
        <li key={note.id}>
          <button
            onClick={() => onSelect(note.id)}
            className={cn(
              "w-full cursor-pointer rounded-lg border px-3 py-2 text-left transition-all duration-150",
              selectedId === note.id
                ? "border-primary/40 bg-primary/12"
                : "border-transparent hover:border-white/10 hover:bg-white/[0.04]"
            )}
          >
            <div className="flex items-center gap-2">
              <FileText
                className={cn(
                  "h-3.5 w-3.5 shrink-0",
                  selectedId === note.id ? "text-primary-3" : "text-text-subtle"
                )}
              />
              <span className="truncate text-[13px] font-medium text-text">
                {note.title || "未命名笔记"}
              </span>
            </div>
            <p className="mt-1 line-clamp-2 pl-[22px] text-[11px] leading-relaxed text-text-subtle">
              {excerpt(note.content, 70)}
            </p>
            <div className="mt-1 pl-[22px] text-[10px] text-text-subtle/70">
              {formatRelative(note.updatedAt)}
            </div>
          </button>
        </li>
      ))}
    </ul>
  );
}
