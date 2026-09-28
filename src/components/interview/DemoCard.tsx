import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { AlertCircle, Loader2, PlayCircle, Sparkles } from "lucide-react";
import { isTauri } from "../../lib/utils";
import { useT } from "../../hooks/useT";
import { useInterviewStore } from "../../stores/interviewStore";

/** 演示链路步骤（展示用） */
const STEPS = [
  "实时转写（面试官 / 我）",
  "问题识别（规则打分 + 合段）",
  "知识库召回（BM25 + 向量融合）",
  "AI 流式回答 + 引用来源",
];

/**
 * 一键全链路演示（无需麦克风 / 模型 / API Key）。
 *
 * 演示只替换「输入源」：问题识别、召回、回答、落库、会话回放全部走真实代码路径。
 */
export default function DemoCard() {
  const startDemoSession = useInterviewStore((s) => s.startDemoSession);
  const stopTranscription = useInterviewStore((s) => s.stopTranscription);
  const startingAsr = useInterviewStore((s) => s.startingAsr);
  const asrError = useInterviewStore((s) => s.asrError);
  const demoOfflineAnswer = useInterviewStore((s) => s.demoOfflineAnswer);
  const [starting, setStarting] = useState(false);
  const t = useT();

  // 演示结束：同步收起会话状态（后端会话记录保留，可在「会话历史」回放）
  useEffect(() => {
    if (!isTauri()) return;
    const unlisten = listen("demo://done", () => {
      stopTranscription();
    });
    return () => {
      unlisten.then((fn) => fn()).catch((err) => console.error(err));
    };
  }, [stopTranscription]);

  const start = () => {
    setStarting(true);
    startDemoSession()
      .catch((err) => console.error("启动演示失败:", err))
      .finally(() => setStarting(false));
  };

  return (
    <section className="glass rounded-xl border-primary/20 p-4 shadow-card">
      <header className="mb-2 flex items-center gap-2">
        <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-gradient-to-br from-primary to-primary-3 shadow-brand">
          <Sparkles className="h-4 w-4 text-white" />
        </div>
        <div className="min-w-0">
          <h2 className="text-[13px] font-semibold">{t("一键演示 · 完整链路")}</h2>
          <p className="text-[11px] text-text-subtle">
            {t("无需麦克风、模型或 API Key，用脚本化输入跑通真实链路")}
          </p>
        </div>
        <button
          onClick={start}
          disabled={starting || startingAsr}
          className="focus-ring ml-auto flex shrink-0 cursor-pointer items-center gap-1.5 rounded-lg border border-primary/40 bg-primary/15 px-3 py-2 text-[12px] text-primary-3 transition-colors hover:bg-primary/25 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {starting || startingAsr ? (
            <Loader2 className="h-3.5 w-3.5 animate-spin" />
          ) : (
            <PlayCircle className="h-3.5 w-3.5" />
          )}
          {t("开始演示")}
        </button>
      </header>

      <ol className="flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-text-muted">
        {STEPS.map((step, index) => (
          <li key={step} className="flex items-center gap-1.5">
            <span className="flex h-4 w-4 items-center justify-center rounded-full bg-ghost/[0.08] text-[9px] text-text-subtle">
              {index + 1}
            </span>
            {t(step)}
          </li>
        ))}
      </ol>

      <p className="mt-2 text-[10px] leading-relaxed text-text-subtle">
        {t(
          "演示会自动写入 4 篇示例笔记用于召回；结束后可在「会话历史」中回放与导出。"
        )}
        {(starting || startingAsr) &&
          ` ${t("正在准备演示数据…")}`}
        {!starting && demoOfflineAnswer && ` ${t("未配置 AI Key，将使用离线拼装回答。")}`}
      </p>

      {asrError && (
        <p className="mt-2 flex items-start gap-1.5 rounded-md border border-danger/30 bg-danger/10 px-3 py-2 text-[11px] leading-relaxed text-danger">
          <AlertCircle className="mt-0.5 h-3 w-3 shrink-0" />
          <span>{asrError}</span>
        </p>
      )}
    </section>
  );
}
