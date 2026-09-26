import { useEffect } from "react";
import {
  AlertCircle,
  CheckCircle2,
  Clock,
  Download,
  FileJson,
  FileText,
  History,
  Loader2,
  RefreshCw,
  Trash2,
  X,
} from "lucide-react";
import { cn, formatClock, formatDateTime } from "../lib/utils";
import { useT } from "../hooks/useT";
import { useSessionStore } from "../stores/sessionStore";

const SPEAKER_LABEL: Record<string, string> = {
  me: "我",
  interviewer: "面试官",
};

export default function SessionsPage() {
  const sessions = useSessionStore((s) => s.sessions);
  const selectedId = useSessionStore((s) => s.selectedId);
  const detail = useSessionStore((s) => s.detail);
  const loading = useSessionStore((s) => s.loading);
  const detailLoading = useSessionStore((s) => s.detailLoading);
  const exporting = useSessionStore((s) => s.exporting);
  const notice = useSessionStore((s) => s.notice);
  const loadSessions = useSessionStore((s) => s.loadSessions);
  const select = useSessionStore((s) => s.select);
  const deleteSession = useSessionStore((s) => s.deleteSession);
  const exportSession = useSessionStore((s) => s.exportSession);
  const clearNotice = useSessionStore((s) => s.clearNotice);

  const t = useT();

  useEffect(() => {
    loadSessions();
  }, [loadSessions]);

  const origin = detail?.session.startedAt ?? 0;

  const handleDelete = () => {
    if (!selectedId) return;
    const title = detail?.session.title ?? selectedId;
    if (window.confirm(`确定删除会话「${title}」？转写与问答将一并删除，且不可恢复。`)) {
      deleteSession(selectedId);
    }
  };

  const iconButton =
    "flex cursor-pointer items-center gap-1.5 rounded-lg border border-white/10 bg-background-3/60 px-2.5 py-1.5 text-[11px] text-text-muted transition-colors hover:border-primary/30 hover:text-primary-3 disabled:cursor-not-allowed disabled:opacity-40";

  return (
    <div className="flex h-full flex-col">
      <div className="flex h-12 shrink-0 items-center gap-3 border-b border-white/5 px-4">
        <History className="h-4 w-4 text-primary-3" />
        <h1 className="text-[15px] font-semibold">{t("会话历史")}</h1>
        <span className="text-[11px] text-text-subtle">
          {t("共 {count} 场", { count: sessions.length })}
        </span>
        <button
          onClick={() => loadSessions()}
          disabled={loading}
          className="ml-auto flex cursor-pointer items-center gap-1.5 rounded-lg border border-white/10 px-2.5 py-1 text-[11px] text-text-muted transition-colors hover:text-text disabled:opacity-50"
        >
          {loading ? (
            <Loader2 className="h-3 w-3 animate-spin" />
          ) : (
            <RefreshCw className="h-3 w-3" />
          )}
          刷新
        </button>
      </div>

      {notice && (
        <div
          className={cn(
            "mx-4 mt-3 flex items-start gap-1.5 rounded-lg border px-3 py-2 text-[11px] leading-relaxed",
            notice.kind === "success"
              ? "border-success/30 bg-success/10 text-success"
              : "border-danger/30 bg-danger/10 text-danger"
          )}
        >
          {notice.kind === "success" ? (
            <CheckCircle2 className="mt-0.5 h-3 w-3 shrink-0" />
          ) : (
            <AlertCircle className="mt-0.5 h-3 w-3 shrink-0" />
          )}
          <span className="min-w-0 break-all">{notice.text}</span>
          <button
            onClick={clearNotice}
            className="ml-auto cursor-pointer shrink-0 opacity-70 hover:opacity-100"
            title="关闭提示"
          >
            <X className="h-3 w-3" />
          </button>
        </div>
      )}

      <div className="flex min-h-0 flex-1 gap-4 overflow-hidden p-4">
        {/* 会话列表 */}
        <aside className="flex w-72 shrink-0 flex-col gap-2 overflow-y-auto">
          {sessions.length === 0 ? (
            <p className="glass rounded-xl px-3 py-6 text-center text-[11px] leading-relaxed text-text-subtle">
              暂无面试会话。
              <br />
              在「面试辅助」中开始转写后，会话会自动记录在此。
            </p>
          ) : (
            sessions.map((session) => (
              <button
                key={session.id}
                onClick={() => select(session.id)}
                className={cn(
                  "glass cursor-pointer rounded-xl p-3 text-left transition-colors",
                  selectedId === session.id
                    ? "border-primary/40 bg-primary/[0.08]"
                    : "hover:bg-white/[0.04]"
                )}
              >
                <div className="truncate text-[12px] font-medium text-text">
                  {session.title}
                </div>
                <div className="mt-1 flex items-center gap-2 text-[10px] text-text-subtle">
                  <span>{formatDateTime(session.startedAt)}</span>
                  {!session.endedAt && (
                    <span className="rounded bg-warning/15 px-1 py-0.5 text-warning">
                      未结束
                    </span>
                  )}
                </div>
                <div className="mt-1.5 text-[10px] text-text-muted">
                  转写 {session.segmentCount} 条 · 问答 {session.questionCount} 组
                </div>
              </button>
            ))
          )}
        </aside>

        {/* 会话详情（回放） */}
        <section className="glass flex min-h-0 min-w-0 flex-1 flex-col rounded-xl p-4">
          {!detail ? (
            <p className="m-auto text-[12px] text-text-subtle">
              {detailLoading ? "正在加载会话详情…" : "选择左侧会话查看完整回放"}
            </p>
          ) : (
            <>
              <header className="mb-3 flex flex-wrap items-center gap-2">
                <h2 className="text-[14px] font-semibold">{detail.session.title}</h2>
                <span className="flex items-center gap-1 text-[10px] text-text-subtle">
                  <Clock className="h-3 w-3" />
                  {formatDateTime(detail.session.startedAt)}
                  {" → "}
                  {detail.session.endedAt
                    ? formatClock(detail.session.endedAt, detail.session.startedAt)
                    : "进行中"}
                </span>

                <div className="ml-auto flex items-center gap-1.5">
                  <button
                    className={iconButton}
                    disabled={exporting}
                    onClick={() => exportSession(detail.session.id, "markdown")}
                  >
                    <FileText className="h-3 w-3" />
                    导出 Markdown
                  </button>
                  <button
                    className={iconButton}
                    disabled={exporting}
                    onClick={() => exportSession(detail.session.id, "json")}
                  >
                    <FileJson className="h-3 w-3" />
                    导出 JSON
                  </button>
                  <button
                    className={cn(iconButton, "hover:border-danger/40 hover:text-danger")}
                    onClick={handleDelete}
                  >
                    <Trash2 className="h-3 w-3" />
                    删除
                  </button>
                </div>
              </header>

              <div className="min-h-0 flex-1 overflow-y-auto pr-1">
                {/* 转写时间线 */}
                <h3 className="mb-1.5 text-[11px] font-medium text-text-muted">
                  转写时间线（{detail.segments.length} 条）
                </h3>
                {detail.segments.length === 0 ? (
                  <p className="rounded-lg border border-white/[0.06] bg-background-3/30 px-3 py-4 text-[11px] text-text-subtle">
                    该会话没有转写记录
                  </p>
                ) : (
                  <ul className="flex flex-col gap-1.5">
                    {detail.segments.map((segment) => (
                      <li key={segment.id} className="flex gap-2">
                        <span className="w-10 shrink-0 pt-0.5 text-[10px] text-text-subtle">
                          {formatClock(segment.ts, origin)}
                        </span>
                        <span
                          className={cn(
                            "mt-0.5 shrink-0 rounded px-1.5 py-0.5 text-[10px]",
                            segment.speaker === "interviewer"
                              ? "bg-primary/15 text-primary-3"
                              : "bg-white/[0.08] text-text-muted"
                          )}
                        >
                          {SPEAKER_LABEL[segment.speaker] ?? segment.speaker}
                        </span>
                        <span className="text-[12px] leading-relaxed text-text">
                          {segment.text}
                        </span>
                      </li>
                    ))}
                  </ul>
                )}

                {/* 问答记录 */}
                <h3 className="mb-1.5 mt-4 text-[11px] font-medium text-text-muted">
                  问答记录（{detail.qa.length} 组）
                </h3>
                {detail.qa.length === 0 ? (
                  <p className="rounded-lg border border-white/[0.06] bg-background-3/30 px-3 py-4 text-[11px] text-text-subtle">
                    该会话没有识别到问题
                  </p>
                ) : (
                  <ul className="flex flex-col gap-2.5">
                    {detail.qa.map((pair, index) => (
                      <li
                        key={pair.id}
                        className="rounded-lg border border-white/[0.06] bg-background-3/30 p-3"
                      >
                        <div className="flex items-start gap-2">
                          <span className="shrink-0 text-[10px] text-text-subtle">
                            {formatClock(pair.ts, origin)}
                          </span>
                          <span className="text-[12px] font-medium leading-relaxed text-text">
                            Q{index + 1}. {pair.question}
                          </span>
                        </div>

                        <p className="mt-1.5 whitespace-pre-wrap text-[12px] leading-relaxed text-text-muted">
                          {pair.answer.trim() || "（未生成回答）"}
                        </p>

                        {pair.sources.length > 0 && (
                          <div className="mt-1.5 flex flex-wrap items-center gap-1">
                            <span className="text-[10px] text-text-subtle">引用：</span>
                            {pair.sources.map((source, sourceIndex) => (
                              <span
                                key={`${source.noteId}-${sourceIndex}`}
                                className="max-w-[12rem] truncate rounded bg-white/[0.06] px-1.5 py-0.5 text-[10px] text-text-muted"
                                title={source.title}
                              >
                                {source.title || "未命名笔记"}
                              </span>
                            ))}
                          </div>
                        )}
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            </>
          )}
        </section>
      </div>

      {/* 页脚：导出格式说明 */}
      <div className="flex shrink-0 items-center gap-1.5 px-4 pb-3 text-[10px] text-text-subtle">
        <Download className="h-3 w-3" />
        导出内容包含完整转写时间线、问答与引用来源；JSON 为结构化完整记录。
      </div>
    </div>
  );
}
