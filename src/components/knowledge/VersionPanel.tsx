import { History, RotateCcw, X } from "lucide-react";
import { excerpt, formatRelative } from "../../lib/utils";
import type { NoteVersion } from "../../types";

interface VersionPanelProps {
  versions: NoteVersion[];
  onRestore: (versionId: string) => void;
  onClose: () => void;
}

/** 右侧版本历史抽屉：列出历史版本并可一键回滚 */
export default function VersionPanel({
  versions,
  onRestore,
  onClose,
}: VersionPanelProps) {
  return (
    <aside className="flex w-72 shrink-0 flex-col border-l border-white/5 bg-background-2/40">
      <div className="flex h-11 shrink-0 items-center gap-2 border-b border-white/5 px-3">
        <History className="h-3.5 w-3.5 text-primary-3" />
        <span className="text-[12px] font-medium">版本历史</span>
        <span className="rounded-full bg-white/[0.06] px-1.5 py-0.5 text-[10px] text-text-muted">
          {versions.length}
        </span>
        <button
          title="关闭版本历史"
          onClick={onClose}
          className="ml-auto cursor-pointer rounded p-1 text-text-subtle transition-colors hover:bg-white/10 hover:text-text"
        >
          <X className="h-3.5 w-3.5" />
        </button>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto p-2">
        {versions.length === 0 ? (
          <p className="px-2 py-6 text-center text-[11px] leading-relaxed text-text-subtle">
            暂无历史版本。
            <br />
            编辑笔记保存后会自动留存快照。
          </p>
        ) : (
          <ul className="flex flex-col gap-1.5">
            {versions.map((version) => (
              <li
                key={version.id}
                className="rounded-lg border border-white/[0.06] bg-background-3/40 p-2.5 transition-colors hover:border-primary/30"
              >
                <div className="flex items-center gap-2">
                  <span className="rounded bg-primary/15 px-1.5 py-0.5 text-[10px] text-primary-3">
                    v{version.version}
                  </span>
                  <span className="text-[10px] text-text-subtle">
                    {formatRelative(version.createdAt)}
                  </span>
                  <button
                    title={`回滚到 v${version.version}`}
                    onClick={() => onRestore(version.id)}
                    className="ml-auto cursor-pointer rounded p-1 text-text-subtle transition-colors hover:bg-primary/20 hover:text-primary-3"
                  >
                    <RotateCcw className="h-3 w-3" />
                  </button>
                </div>
                <p className="mt-1.5 text-[11px] font-medium text-text-muted">
                  {version.title || "未命名笔记"}
                </p>
                <p className="mt-0.5 line-clamp-2 text-[10px] leading-relaxed text-text-subtle">
                  {excerpt(version.content, 60) || "（空白内容）"}
                </p>
              </li>
            ))}
          </ul>
        )}
      </div>
    </aside>
  );
}
