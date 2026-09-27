import {
  AlertCircle,
  Brain,
  HardDrive,
  Loader2,
  Trash2,
  Waves,
} from "lucide-react";
import { cn, formatBytes } from "../../lib/bytes";
import type { ModelKind, ModelProgress, ModelStatus } from "../../types";

const KIND_META: Record<ModelKind, { label: string; icon: typeof Brain }> = {
  embedding: { label: "嵌入模型", icon: Brain },
  vad: { label: "VAD", icon: Waves },
  asr: { label: "本地 ASR", icon: HardDrive },
};

interface ModelManagerSectionProps {
  models: ModelStatus[];
  progress: Record<string, ModelProgress>;
  downloadingId: string | null;
  onDownload: (id: string) => void;
  onDelete: (id: string) => void;
  /** 各模型下载失败信息 */
  errors?: Record<string, string>;
}

export default function ModelManagerSection({
  models,
  progress,
  downloadingId,
  onDownload,
  onDelete,
  errors = {},
}: ModelManagerSectionProps) {
  return (
    <section className="glass rounded-xl p-4 shadow-card">
      <header className="mb-4 flex items-center gap-2">
        <h2 className="text-[13px] font-semibold">模型管理</h2>
        <span className="text-[11px] text-text-subtle">
          模型按需下载，不包含在安装包内
        </span>
      </header>

      <ul className="flex flex-col gap-2">
        {models.map((model) => {
          const meta = KIND_META[model.kind];
          const Icon = meta.icon;
          const current = progress[model.id];
          const downloading = downloadingId === model.id;
          const error = errors[model.id];
          const percent =
            current && current.total
              ? Math.min(100, Math.round((current.downloaded / current.total) * 100))
              : null;

          return (
            <li
              key={model.id}
              className="rounded-md border border-white/[0.06] bg-background-3/40 p-3"
            >
              <div className="flex items-start gap-3">
                <Icon className="mt-0.5 h-4 w-4 shrink-0 text-primary-3" />
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="text-[12px] font-medium text-text">
                      {model.name}
                    </span>
                    <span className="rounded bg-white/[0.06] px-1.5 py-0.5 text-[10px] text-text-subtle">
                      {meta.label}
                    </span>
                    {model.installed && (
                      <span className="rounded bg-success/15 px-1.5 py-0.5 text-[10px] text-success">
                        已安装
                      </span>
                    )}
                    {!model.installed && model.partialBytes > 0 && (
                      <span className="rounded bg-warning/15 px-1.5 py-0.5 text-[10px] text-warning">
                        已缓存 {formatBytes(model.partialBytes)}
                      </span>
                    )}
                  </div>
                  <p className="mt-1 text-[11px] leading-relaxed text-text-muted">
                    {model.description}
                  </p>
                  {model.installed && model.resolvedPath && (
                    <p className="mt-0.5 truncate text-[10px] text-text-subtle">
                      {model.resolvedPath}
                    </p>
                  )}
                </div>

                <div className="flex shrink-0 items-center gap-1">
                  <button
                    onClick={() => onDownload(model.id)}
                    disabled={downloading}
                    className="focus-ring cursor-pointer rounded-lg border border-primary/40 bg-primary/15 px-3 py-1.5 text-[11px] text-primary-3 transition-colors hover:bg-primary/25 disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    {downloading ? "下载中…" : model.installed ? "重新下载" : "下载"}
                  </button>
                  {(model.installed || model.partialBytes > 0) && (
                    <button
                      title="删除本地文件"
                      onClick={() => onDelete(model.id)}
                      className="focus-ring cursor-pointer rounded-lg border border-white/10 p-1.5 text-text-subtle transition-colors hover:border-danger/40 hover:text-danger"
                    >
                      <Trash2 className="h-3.5 w-3.5" />
                    </button>
                  )}
                </div>
              </div>

              {downloading && (
                <div className="mt-2">
                  <div className="h-1.5 w-full overflow-hidden rounded-full bg-white/[0.06]">
                    <div
                      className={cn(
                        "h-full rounded-full bg-gradient-to-r from-primary to-primary-3 transition-[width] duration-200",
                        percent === null && "w-1/3 animate-pulse"
                      )}
                      style={percent !== null ? { width: `${percent}%` } : undefined}
                    />
                  </div>
                  <p className="mt-1 flex items-center gap-1 text-[10px] text-text-subtle">
                    <Loader2 className="h-3 w-3 animate-spin" />
                    {current?.total
                      ? `${formatBytes(current.downloaded)} / ${formatBytes(current.total)}（${percent}%）`
                      : "正在连接下载源…"}
                  </p>
                </div>
              )}

              {error && !downloading && (
                <p className="mt-2 flex items-start gap-1.5 rounded-md border border-danger/30 bg-danger/10 px-3 py-2 text-[11px] leading-relaxed text-danger">
                  <AlertCircle className="mt-0.5 h-3 w-3 shrink-0" />
                  <span>{error}</span>
                </p>
              )}
            </li>
          );
        })}
      </ul>
    </section>
  );
}
