import { useState } from "react";
import { Hash, X } from "lucide-react";

interface TagEditorProps {
  tags: string[];
  /** 已有标签，作为输入建议 */
  suggestions: string[];
  onAdd: (tag: string) => void;
  onRemove: (tag: string) => void;
}

/** 笔记标签编辑器：回车添加、点击 × 移除 */
export default function TagEditor({
  tags,
  suggestions,
  onAdd,
  onRemove,
}: TagEditorProps) {
  const [draft, setDraft] = useState("");

  const commit = () => {
    const name = draft.trim();
    setDraft("");
    if (!name || tags.includes(name)) return;
    onAdd(name);
  };

  return (
    <div className="flex flex-wrap items-center gap-1.5 border-b border-white/[0.06] px-4 py-1.5">
      <Hash className="h-3 w-3 shrink-0 text-text-subtle" />

      {tags.map((tag) => (
        <span
          key={tag}
          className="flex items-center gap-1 rounded-full border border-primary/25 bg-primary/10 px-2 py-0.5 text-[11px] text-primary-3"
        >
          {tag}
          <button
            title={`移除标签 ${tag}`}
            onClick={() => onRemove(tag)}
            className="focus-ring cursor-pointer rounded text-primary-3/70 transition-colors hover:text-danger"
          >
            <X className="h-3 w-3" />
          </button>
        </span>
      ))}

      <input
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") commit();
          if (e.key === "Escape") setDraft("");
        }}
        onBlur={commit}
        placeholder="添加标签…"
        list="tag-suggestions"
        className="w-28 border-0 bg-transparent p-0 text-[11px] text-text outline-none placeholder:text-text-subtle"
      />
      <datalist id="tag-suggestions">
        {suggestions
          .filter((s) => !tags.includes(s))
          .map((s) => (
            <option key={s} value={s} />
          ))}
      </datalist>
    </div>
  );
}
