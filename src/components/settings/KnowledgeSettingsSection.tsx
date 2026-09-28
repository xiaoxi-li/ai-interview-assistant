import { useEffect, useMemo, useState } from "react";
import { AlertCircle, Database, FolderOpen, RefreshCw, Save } from "lucide-react";
import { cn } from "../../lib/utils";
import { SaveFeedback } from "./SaveFeedback";
import { pickDirectory } from "../../lib/dialog";
import type { KnowledgeConfig, ModelStatus, StoragePaths } from "../../types";

const FIELD =
  "focus-ring w-full rounded-lg border border-ghost/10 bg-background-3/60 px-3 py-2 text-[12px] text-text outline-none transition-colors placeholder:text-text-subtle focus:border-primary/50";
const LABEL = "mb-1.5 block text-[11px] text-text-muted";

interface KnowledgeSettingsSectionProps {
  config: KnowledgeConfig;
  storagePaths: StoragePaths | null;
  models: ModelStatus[];
  onSave: (config: KnowledgeConfig) => void;
}

export default function KnowledgeSettingsSection({
  config,
  storagePaths,
  models,
  onSave,
}: KnowledgeSettingsSectionProps) {
  const [draft, setDraft] = useState<KnowledgeConfig>(config);

  useEffect(() => {
    setDraft(config);
  }, [config]);

  const embeddingModels = useMemo(
    () => models.filter((m) => m.kind === "embedding"),
    [models]
  );

  const pathChanged =
    (draft.dbPath ?? null) !== (config.dbPath ?? null) ||
    (draft.modelsDir ?? null) !== (config.modelsDir ?? null);

  const browse = (field: "dbPath" | "modelsDir") => {
    pickDirectory().then((dir) => {
      if (!dir) return;
      setDraft((prev) =>
        field === "dbPath" ? { ...prev, dbPath: dir } : { ...prev, modelsDir: dir }
      );
    });
  };

  const resetPath = (field: "dbPath" | "modelsDir") => {
    setDraft((prev) => ({ ...prev, [field]: null }));
  };

  return (
    <section className="glass rounded-xl p-4 shadow-card">
      <header className="mb-4 flex items-center gap-2">
        <h2 className="text-[13px] font-semibold">知识库</h2>
        <span className="text-[11px] text-text-subtle">
          存储位置与本地嵌入模型
        </span>
      </header>

      <div className="flex flex-col gap-3">
        {/* 数据库路径 */}
        <div>
          <label className={LABEL} htmlFor="kb-db-path">
            数据库路径
          </label>
          <div className="flex items-center gap-2">
            <div className="flex min-w-0 flex-1 items-center gap-2">
              <Database className="h-3.5 w-3.5 shrink-0 text-text-subtle" />
              <input
                id="kb-db-path"
                value={draft.dbPath ?? ""}
                onChange={(e) =>
                  setDraft({ ...draft, dbPath: e.target.value || null })
                }
                placeholder={storagePaths?.dbPath ?? "默认应用数据目录"}
                className={FIELD}
              />
            </div>
            <button
              onClick={() => browse("dbPath")}
              title="选择目录"
              className="focus-ring cursor-pointer rounded-lg border border-ghost/10 bg-background-3/60 p-2 text-text-muted transition-colors hover:border-primary/40 hover:text-text"
            >
              <FolderOpen className="h-3.5 w-3.5" />
            </button>
            {draft.dbPath && (
              <button
                onClick={() => resetPath("dbPath")}
                title="恢复默认路径"
                className="focus-ring cursor-pointer rounded-lg border border-ghost/10 bg-background-3/60 p-2 text-text-muted transition-colors hover:text-text"
              >
                <RefreshCw className="h-3.5 w-3.5" />
              </button>
            )}
          </div>
        </div>

        {/* 模型目录 */}
        <div>
          <label className={LABEL} htmlFor="kb-models-dir">
            模型目录
          </label>
          <div className="flex items-center gap-2">
            <div className="flex min-w-0 flex-1 items-center gap-2">
              <FolderOpen className="h-3.5 w-3.5 shrink-0 text-text-subtle" />
              <input
                id="kb-models-dir"
                value={draft.modelsDir ?? ""}
                onChange={(e) =>
                  setDraft({ ...draft, modelsDir: e.target.value || null })
                }
                placeholder={storagePaths?.modelsDir ?? "默认应用数据目录"}
                className={FIELD}
              />
            </div>
            <button
              onClick={() => browse("modelsDir")}
              title="选择目录"
              className="focus-ring cursor-pointer rounded-lg border border-ghost/10 bg-background-3/60 p-2 text-text-muted transition-colors hover:border-primary/40 hover:text-text"
            >
              <FolderOpen className="h-3.5 w-3.5" />
            </button>
            {draft.modelsDir && (
              <button
                onClick={() => resetPath("modelsDir")}
                title="恢复默认路径"
                className="focus-ring cursor-pointer rounded-lg border border-ghost/10 bg-background-3/60 p-2 text-text-muted transition-colors hover:text-text"
              >
                <RefreshCw className="h-3.5 w-3.5" />
              </button>
            )}
          </div>
        </div>

        {/* 嵌入模型 */}
        <div>
          <label className={LABEL} htmlFor="kb-embedding-model">
            嵌入模型
          </label>
          <select
            id="kb-embedding-model"
            value={draft.embeddingModel}
            onChange={(e) =>
              setDraft({ ...draft, embeddingModel: e.target.value })
            }
            className={cn(FIELD, "cursor-pointer")}
          >
            {embeddingModels.length === 0 && (
              <option value={draft.embeddingModel}>{draft.embeddingModel}</option>
            )}
            {embeddingModels.map((model) => (
              <option key={model.id} value={model.id}>
                {model.name}
                {model.installed ? "（已安装）" : "（未下载）"}
              </option>
            ))}
          </select>
          <p className="mt-1 text-[10px] text-text-subtle">
            本地嵌入模型保证知识库离线可用与检索低延迟
          </p>
        </div>
      </div>

      {pathChanged && (
        <p className="mt-3 flex items-center gap-1.5 rounded-md border border-warning/30 bg-warning/10 px-3 py-2 text-[11px] text-warning">
          <AlertCircle className="h-3 w-3 shrink-0" />
          路径变更需重启应用后生效，现有数据不会自动迁移
        </p>
      )}

      <div className="mt-4 flex items-center justify-end gap-3">
        {/* 保存反馈：知识库配置保存 */}
        <SaveFeedback target="knowledge" />
        <button
          onClick={() => onSave(draft)}
          className="focus-ring flex cursor-pointer items-center gap-1.5 rounded-lg border border-primary/40 bg-primary/15 px-4 py-2 text-[12px] text-primary-3 transition-colors hover:bg-primary/25"
        >
          <Save className="h-3.5 w-3.5" />
          保存配置
        </button>
      </div>
    </section>
  );
}
