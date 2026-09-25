import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";
import { Clock, FileText, Hash } from "lucide-react";
import { formatTime } from "../../lib/utils";
import type { Note } from "../../types";

interface NotePreviewProps {
  note: Note | null;
}

export default function NotePreview({ note }: NotePreviewProps) {
  if (!note) {
    return (
      <div className="flex h-full items-center justify-center">
        <p className="text-[12px] text-text-subtle">选择左侧笔记查看内容</p>
      </div>
    );
  }

  return (
    <div className="selectable flex h-full flex-col">
      <div className="flex h-11 shrink-0 items-center gap-2 border-b border-white/5 px-4">
        <FileText className="h-3.5 w-3.5 text-primary-3" />
        <h2 className="truncate text-[14px] font-semibold">{note.title}</h2>
        <div className="ml-auto flex items-center gap-3 text-[11px] text-text-subtle">
          {note.tags.length > 0 && (
            <span className="flex items-center gap-1">
              <Hash className="h-3 w-3" />
              {note.tags.join(" / ")}
            </span>
          )}
          <span className="flex items-center gap-1">
            <Clock className="h-3 w-3" />
            {formatTime(note.updatedAt)}
          </span>
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-6 py-4">
        <div className="markdown-body mx-auto max-w-3xl">
          <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
            {note.content}
          </ReactMarkdown>
        </div>
      </div>
    </div>
  );
}
