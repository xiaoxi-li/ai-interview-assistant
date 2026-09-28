import { useEffect, useState } from "react";
import { AlertCircle, CheckCircle2, Download, Loader2, RefreshCw, Rocket } from "lucide-react";
import { cn } from "../../lib/utils";
import { ipc } from "../../lib/ipc";
import { useT } from "../../hooks/useT";
import { SaveFeedbackView } from "./SaveFeedback";
import type { SettingsSaveFeedback } from "../../stores/settingsStore";
import type { UpdateCheck, UpdateConfig } from "../../types";

const FIELD =
  "focus-ring w-full rounded-lg border border-white/10 bg-background-3/60 px-3 py-2 text-[13px] text-text outline-none transition-colors placeholder:text-text-subtle focus:border-primary/50";

/** 关于与更新（P2）：版本信息、更新地址与检查更新 */
export default function UpdateSection() {
  const [version, setVersion] = useState("");
  const [config, setConfig] = useState<UpdateConfig | null>(null);
  const [draftEndpoint, setDraftEndpoint] = useState("");
  const [checking, setChecking] = useState(false);
  const [result, setResult] = useState<UpdateCheck | null>(null);
  const [error, setError] = useState<string | null>(null);
  /** 保存反馈（ST-11）：本分区直连 ipc，不走 settingsStore，故用本地状态 */
  const [feedback, setFeedback] = useState<SettingsSaveFeedback | null>(null);
  const t = useT();

  useEffect(() => {
    ipc<string>("app_version")
      .then(setVersion)
      .catch((err) => console.error("读取版本失败:", err));

    ipc<UpdateConfig>("get_update_config")
      .then((loaded) => {
        setConfig(loaded);
        setDraftEndpoint(loaded.endpoint);
      })
      .catch((err) => {
        console.error("读取更新配置失败:", err);
        setError(String(err));
      });
  }, []);

  const saveEndpoint = () => {
    if (!config) return;
    setError(null);
    setFeedback(null);
    ipc<UpdateConfig>("save_update_config", {
      config: { ...config, endpoint: draftEndpoint.trim() },
    })
      .then((saved) => {
        setConfig(saved);
        setFeedback({ kind: "success", text: "更新地址已保存" });
      })
      .catch((err) => {
        console.error("保存更新配置失败:", err);
        setError(String(err));
        setFeedback({
          kind: "error",
          text: `保存更新地址失败：${err instanceof Error ? err.message : String(err)}`,
        });
      });
  };

  const check = () => {
    setChecking(true);
    setError(null);
    setResult(null);
    ipc<UpdateCheck>("check_app_update", {
      endpoint: draftEndpoint.trim() || null,
      currentVersion: null,
    })
      .then((outcome) => setResult(outcome))
      .catch((err) => {
        console.error("检查更新失败:", err);
        setError(String(err));
      })
      .finally(() => setChecking(false));
  };

  return (
    <section className="glass rounded-xl p-4 shadow-card">
      <header className="mb-3 flex items-center gap-2">
        <Rocket className="h-4 w-4 text-primary-3" />
        <h2 className="text-[13px] font-semibold">{t("关于与更新")}</h2>
        <span className="ml-auto rounded-full bg-white/[0.06] px-2 py-0.5 text-[10px] text-text-subtle">
          v{version || "…"}
        </span>
      </header>

      <label className="mb-1.5 block text-[11px] text-text-muted" htmlFor="update-endpoint">
        {t("更新地址（latest.json）")}
      </label>
      <input
        id="update-endpoint"
        className={FIELD}
        value={draftEndpoint}
        placeholder="https://example.com/releases/latest.json"
        onChange={(event) => setDraftEndpoint(event.target.value)}
        onBlur={saveEndpoint}
      />
      <p className="mt-1.5 text-[10px] leading-relaxed text-text-subtle">
        {t("留空则关闭更新检查；填写后可由发布流水线生成的 latest.json 提供版本信息。")}
      </p>

      <div className="mt-3 flex items-center gap-2">
        <button
          onClick={check}
          disabled={checking}
          className="focus-ring flex cursor-pointer items-center gap-1.5 rounded-lg border border-primary/40 bg-primary/15 px-3 py-1.5 text-[11px] text-primary-3 transition-colors hover:bg-primary/25 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {checking ? (
            <Loader2 className="h-3 w-3 animate-spin" />
          ) : (
            <RefreshCw className="h-3 w-3" />
          )}
          {t("检查更新")}
        </button>
        {config && (
          <label className="flex cursor-pointer items-center gap-1.5 text-[11px] text-text-muted">
            <input
              type="checkbox"
              checked={config.autoCheck}
              onChange={(event) => {
                const next = { ...config, autoCheck: event.target.checked };
                setConfig(next);
                setFeedback(null);
                ipc<UpdateConfig>("save_update_config", { config: next })
                  .then(() =>
                    setFeedback({ kind: "success", text: "自动检查设置已保存" })
                  )
                  .catch((err) => {
                    console.error("保存更新配置失败:", err);
                    setFeedback({
                      kind: "error",
                      text: `保存自动检查设置失败：${
                        err instanceof Error ? err.message : String(err)
                      }`,
                    });
                  });
              }}
              className="focus-ring cursor-pointer rounded accent-[#4F6BED]"
            />
            {t("启动时自动检查")}
          </label>
        )}
        {/* 保存反馈：更新地址失焦保存 / 自动检查开关 */}
        <SaveFeedbackView feedback={feedback} onDismiss={() => setFeedback(null)} />
      </div>

      {error && (
        <p className="mt-3 flex items-start gap-1.5 rounded-md border border-danger/30 bg-danger/10 px-3 py-2 text-[11px] leading-relaxed text-danger">
          <AlertCircle className="mt-0.5 h-3 w-3 shrink-0" />
          <span>{error}</span>
        </p>
      )}

      {result && (
        <div
          className={cn(
            "mt-3 rounded-md border px-3 py-2",
            result.available
              ? "border-primary/30 bg-primary/[0.08]"
              : "border-success/30 bg-success/10"
          )}
        >
          <p className="flex items-center gap-1.5 text-[11px] font-medium">
            {result.available ? (
              <Download className="h-3 w-3 text-primary-3" />
            ) : (
              <CheckCircle2 className="h-3 w-3 text-success" />
            )}
            {result.message}
          </p>
          {result.notes && (
            <p className="mt-1 whitespace-pre-wrap text-[11px] leading-relaxed text-text-muted">
              {result.notes}
            </p>
          )}
          {result.url && (
            <p className="mt-1 break-all text-[10px] text-text-subtle">{result.url}</p>
          )}
        </div>
      )}
    </section>
  );
}
