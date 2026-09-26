import { Check, Code2, Columns2, Eye, History, Loader2 } from "lucide-react";
import { cn, formatTime } from "../../lib/utils";
import type { ViewMode } from "../../types";

const MODES: { key: ViewMode; label: string; icon: typeof Code2 }[] = [
  { key: "source", label: "源码", icon: Code2 },
  { key: "split", label: "分栏", icon: Columns2 },
  { key: "preview", label: "预览", icon: Eye },
];

interface EditorToolbarProps {
  title: string;
  viewMode: ViewMode;
  saving: boolean;
  savedAt: number | null;
  onTitleChange: (value: string) => void;
  onViewModeChange: (mode: ViewMode) => void;
  /** 版本历史面板是否展开 */
  historyOpen?: boolean;
  onToggleHistory?: () => void;
}

export default function EditorToolbar({
  title,
  viewMode,
  saving,
  savedAt,
  onTitleChange,
  onViewModeChange,
  historyOpen = false,
  onToggleHistory,
}: EditorToolbarProps) {
  return (
    <div className="flex h-11 shrink-0 items-center gap-3 border-b border-white/5 px-4">
      <input
        value={title}
        onChange={(e) => onTitleChange(e.target.value)}
        placeholder="笔记标题"
        className="min-w-0 flex-1 border-0 bg-transparent p-0 text-[14px] font-semibold text-text outline-none placeholder:text-text-subtle"
      />

      <span className="flex items-center gap-1 text-[11px] text-text-subtle">
        {saving ? (
          <>
            <Loader2 className="h-3 w-3 animate-spin" />
            保存中…
          </>
        ) : savedAt ? (
          <>
            <Check className="h-3 w-3 text-success" />
            已保存 {formatTime(savedAt)}
          </>
        ) : null}
      </span>

      {onToggleHistory && (
        <button
          title="版本历史"
          onClick={onToggleHistory}
          className={cn(
            "flex cursor-pointer items-center rounded-md border border-white/10 p-1.5 transition-colors",
            historyOpen
              ? "bg-primary/20 text-primary-3"
              : "bg-background-3/60 text-text-subtle hover:text-text"
          )}
        >
          <History className="h-3.5 w-3.5" />
        </button>
      )}

      <div className="flex items-center gap-0.5 rounded-lg border border-white/10 bg-background-3/60 p-0.5">
        {MODES.map(({ key, label, icon: Icon }) => (
          <button
            key={key}
            onClick={() => onViewModeChange(key)}
            title={label}
            className={cn(
              "flex cursor-pointer items-center gap-1 rounded-md px-2 py-1 text-[11px] transition-colors",
              viewMode === key
                ? "bg-primary/20 text-primary-3"
                : "text-text-subtle hover:text-text-muted"
            )}
          >
            <Icon className="h-3.5 w-3.5" />
            {label}
          </button>
        ))}
      </div>
    </div>
  );
}
