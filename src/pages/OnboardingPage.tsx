import { useEffect, useMemo, useState } from "react";
import {
  Check,
  ChevronLeft,
  ChevronRight,
  Cloud,
  Download,
  Loader2,
  Mic,
  MonitorPlay,
  ShieldCheck,
  Sparkles,
  Waves,
} from "lucide-react";
import { cn } from "../lib/utils";
import logoLockupLight from "../assets/logo-lockup-light.png";
import logoLockupDark from "../assets/logo-lockup-dark.png";
import { useModelProgress } from "../hooks/useModelProgress";
import { useAppStore } from "../stores/appStore";
import { useSettingsStore } from "../stores/settingsStore";
import AiSettingsSection from "../components/settings/AiSettingsSection";

const STEPS = [
  { key: "intro", title: "本地优先与权限", icon: ShieldCheck },
  { key: "asr", title: "语音识别通路", icon: Waves },
  { key: "ai", title: "AI 平台配置", icon: Sparkles },
  { key: "done", title: "完成", icon: Check },
] as const;

/** 本地 ASR 模型在模型清单中的标记：requiredForLocalAsr */
export default function OnboardingPage() {
  const [step, setStep] = useState(0);

  const completeOnboarding = useAppStore((s) => s.completeOnboarding);

  const aiConfig = useSettingsStore((s) => s.aiConfig);
  const presets = useSettingsStore((s) => s.presets);
  const apiKeys = useSettingsStore((s) => s.apiKeys);
  const testing = useSettingsStore((s) => s.testing);
  const testResult = useSettingsStore((s) => s.testResult);
  const loadAiSettings = useSettingsStore((s) => s.loadAiSettings);
  const saveAiConfig = useSettingsStore((s) => s.saveAiConfig);
  const saveApiKey = useSettingsStore((s) => s.saveApiKey);
  const deleteApiKey = useSettingsStore((s) => s.deleteApiKey);
  const testAiConnection = useSettingsStore((s) => s.testAiConnection);
  const loadApiKeyStatus = useSettingsStore((s) => s.loadApiKeyStatus);

  const models = useSettingsStore((s) => s.models);
  const modelProgress = useSettingsStore((s) => s.modelProgress);
  const downloadingId = useSettingsStore((s) => s.downloadingId);
  const loadModels = useSettingsStore((s) => s.loadModels);
  const downloadModel = useSettingsStore((s) => s.downloadModel);
  const applyModelProgress = useSettingsStore((s) => s.applyModelProgress);

  useModelProgress(applyModelProgress);

  useEffect(() => {
    loadAiSettings();
    loadModels();
  }, [loadAiSettings, loadModels]);

  const platform = aiConfig?.platform;
  useEffect(() => {
    if (platform) loadApiKeyStatus(platform);
  }, [platform, loadApiKeyStatus]);

  const asrModel = useMemo(
    () => models.find((model) => model.requiredForLocalAsr),
    [models]
  );
  const progress = asrModel ? modelProgress[asrModel.id] : undefined;
  const downloading = Boolean(asrModel && downloadingId === asrModel.id);
  const percent = progress?.total
    ? Math.min(100, Math.round((progress.downloaded / progress.total) * 100))
    : 0;
  const progressText = progress
    ? progress.total
      ? `已下载 ${(progress.downloaded / 1024 / 1024).toFixed(1)} MB / ${(
          progress.total /
          1024 /
          1024
        ).toFixed(1)} MB（${percent}%）`
      : `已下载 ${(progress.downloaded / 1024 / 1024).toFixed(1)} MB`
    : "";

  const current = STEPS[step];
  const isLast = step === STEPS.length - 1;

  return (
    <div className="flex h-full w-full items-center justify-center bg-background p-6">
      <div className="glass flex w-full max-w-3xl flex-col rounded-2xl p-6 shadow-card">
        {/* 步骤指示 */}
        <header className="mb-5 flex items-center gap-3">
          <img
            src={logoLockupLight}
            alt=""
            draggable={false}
            className="h-10 w-auto select-none dark:hidden"
          />
          <img
            src={logoLockupDark}
            alt=""
            draggable={false}
            className="hidden h-10 w-auto select-none dark:block"
          />
          <div>
            <h1 className="text-[15px] font-semibold">欢迎使用轻松面试</h1>
            <p className="text-[11px] text-text-subtle">
              第 {step + 1} / {STEPS.length} 步 · {current.title}
            </p>
          </div>
          <button
            onClick={() => completeOnboarding()}
            className="focus-ring ml-auto cursor-pointer rounded-lg border border-ghost/10 px-2.5 py-1 text-[11px] text-text-muted transition-colors hover:text-text"
          >
            跳过向导
          </button>
        </header>

        <ol className="mb-5 flex items-center gap-2">
          {STEPS.map((item, index) => (
            <li key={item.key} className="flex flex-1 items-center gap-2">
              <span
                className={cn(
                  "flex h-6 w-6 shrink-0 items-center justify-center rounded-full text-[11px]",
                  index < step
                    ? "bg-success/20 text-success"
                    : index === step
                      ? "bg-primary/20 text-primary-3"
                      : "bg-ghost/[0.06] text-text-subtle"
                )}
              >
                {index < step ? <Check className="h-3 w-3" /> : index + 1}
              </span>
              <span
                className={cn(
                  "truncate text-[11px]",
                  index === step ? "text-text" : "text-text-subtle"
                )}
              >
                {item.title}
              </span>
              {index < STEPS.length - 1 && (
                <span className="h-px flex-1 bg-ghost/10" />
              )}
            </li>
          ))}
        </ol>

        <div className="min-h-[260px] flex-1 rounded-xl border border-ghost/[0.06] bg-background-3/30 p-4">
          {step === 0 && (
            <div className="flex flex-col gap-3">
              <h2 className="text-[13px] font-semibold">面试内容只保留在这台电脑上</h2>
              <ul className="flex flex-col gap-2 text-[12px] leading-relaxed text-text-muted">
                <li className="flex gap-2">
                  <ShieldCheck className="mt-0.5 h-3.5 w-3.5 shrink-0 text-success" />
                  默认使用<b>云端语音识别</b>（阿里云百炼）：音频会发送至该服务商；如需音频不出本机，
                  可在「设置 → 语音识别」切到<b>本地</b>通路，或在「设置 → 隐私与安全」取消上传。
                </li>
                <li className="flex gap-2">
                  <Mic className="mt-0.5 h-3.5 w-3.5 shrink-0 text-primary-3" />
                  首次开始转写时，系统会弹出<b>麦克风</b>授权请求，请选择允许。
                </li>
                <li className="flex gap-2">
                  <MonitorPlay className="mt-0.5 h-3.5 w-3.5 shrink-0 text-warning" />
                  macOS 需在「系统设置 → 隐私与安全性 → <b>屏幕录制</b>」中授权本应用，
                  才能采集系统音频（面试官声音）；Windows 需启用「立体声混音」等回环设备。
                </li>
                <li className="flex gap-2">
                  <Sparkles className="mt-0.5 h-3.5 w-3.5 shrink-0 text-primary-3" />
                  AI 回答由你自行配置的大模型服务生成，API Key 保存在系统钥匙链中，不会明文落盘。
                </li>
              </ul>
            </div>
          )}

          {step === 1 && (
            <div className="flex flex-col gap-3">
              <h2 className="text-[13px] font-semibold">默认使用云端语音识别</h2>
              <p className="text-[12px] leading-relaxed text-text-muted">
                开箱默认走<b>云端实时识别</b>（阿里云百炼）：在「设置 → 语音识别」填入百炼 API Key
                即可开始转写，隐私设置已默认允许上传音频。如需音频不出本机，可改为<b>本地</b>识别
                （sherpa-onnx 中文模型，需先下载模型）。
              </p>

              <div className="rounded-md border border-ghost/[0.06] bg-background-2/40 p-3">
                <div className="flex items-center gap-2">
                  <Cloud className="h-3.5 w-3.5 text-text-subtle" />
                  <span className="text-[12px] font-medium">
                    {asrModel?.name ?? "本地语音识别模型"}
                  </span>
                  <span
                    className={cn(
                      "ml-auto rounded-full px-2 py-0.5 text-[10px]",
                      asrModel?.installed
                        ? "bg-success/15 text-success"
                        : "bg-warning/15 text-warning"
                    )}
                  >
                    {asrModel?.installed ? "已就绪" : "待下载"}
                  </span>
                </div>
                {asrModel?.description && (
                  <p className="mt-1 text-[11px] leading-relaxed text-text-subtle">
                    {asrModel.description}
                  </p>
                )}

                {progress && (
                  <div className="mt-2">
                    <div className="h-1 overflow-hidden rounded-full bg-ghost/10">
                      <div
                        className="h-full rounded-full bg-primary transition-all"
                        style={{ width: `${percent}%` }}
                      />
                    </div>
                    <p className="mt-1 text-[10px] text-text-subtle">{progressText}</p>
                  </div>
                )}

                {asrModel && !asrModel.installed && !downloading && (
                  <button
                    onClick={() => downloadModel(asrModel.id)}
                    className="focus-ring mt-2 flex cursor-pointer items-center gap-1.5 rounded-lg border border-primary/40 bg-primary/15 px-3 py-1.5 text-[11px] text-primary-3 transition-colors hover:bg-primary/25"
                  >
                    <Download className="h-3 w-3" />
                    立即下载模型
                  </button>
                )}
                {downloading && (
                  <p className="mt-2 flex items-center gap-1.5 text-[11px] text-text-muted">
                    <Loader2 className="h-3 w-3 animate-spin" />
                    正在下载，可稍后在「设置 → 模型管理」继续
                  </p>
                )}
              </div>

              <p className="text-[11px] leading-relaxed text-text-subtle">
                提示：本地通路为可选项，模型文件较大，可跳过此步——需要离线识别时再到
                「设置 → 模型管理」下载即可。
              </p>
            </div>
          )}

          {step === 2 && (
            <div className="flex flex-col gap-3">
              <h2 className="text-[13px] font-semibold">配置一个大模型服务</h2>
              <p className="text-[12px] leading-relaxed text-text-muted">
                用于生成口述式回答，支持 DeepSeek / Qwen / GLM / 硅基流动 / OpenAI 等 OpenAI 兼容服务。
                密钥存入系统钥匙链，界面只回显掩码。
              </p>
              {aiConfig ? (
                <AiSettingsSection
                  config={aiConfig}
                  presets={presets}
                  apiKeyStatus={apiKeys[aiConfig.platform]}
                  onSaveConfig={saveAiConfig}
                  onSaveApiKey={saveApiKey}
                  onDeleteApiKey={deleteApiKey}
                  onTestConnection={testAiConnection}
                  testing={testing}
                  testResult={testResult}
                />
              ) : (
                <p className="text-[11px] text-text-subtle">正在加载 AI 配置…</p>
              )}
            </div>
          )}

          {step === 3 && (
            <div className="flex flex-col gap-3">
              <h2 className="text-[13px] font-semibold">准备就绪</h2>
              <ul className="flex flex-col gap-2 text-[12px] leading-relaxed text-text-muted">
                <li>
                  · 本地语音识别（可选）：
                  <b className={asrModel?.installed ? "text-success" : "text-warning"}>
                    {asrModel?.installed ? "模型已就绪" : "尚未下载模型（可稍后下载）"}
                  </b>
                </li>
                <li>
                  · AI 平台：
                  <b>
                    {aiConfig
                      ? `${aiConfig.platform} / ${aiConfig.model}`
                      : "尚未配置（可稍后配置）"}
                  </b>
                  {platform && apiKeys[platform] && (
                    <span className="ml-1 text-text-subtle">
                      （密钥{apiKeys[platform].configured ? "已保存" : "未保存"}）
                    </span>
                  )}
                </li>
                <li>
                  · 下一步：导入 Markdown 笔记构建知识库，然后在「面试辅助」中选择
                  <b> 双通道（区分说话人）</b>并开始转写。
                </li>
              </ul>
              <p className="rounded-md border border-ghost/[0.06] bg-background-2/40 px-3 py-2 text-[11px] leading-relaxed text-text-subtle">
                所有设置随时可在「设置」页调整；按 <b>Ctrl/Cmd + Shift + A</b> 可在面试中一键提问。
              </p>
            </div>
          )}
        </div>

        <footer className="mt-5 flex items-center gap-2">
          <button
            onClick={() => setStep((value) => Math.max(0, value - 1))}
            disabled={step === 0}
            className="focus-ring flex cursor-pointer items-center gap-1 rounded-lg border border-ghost/10 bg-background-3/60 px-3 py-2 text-[12px] text-text-muted transition-colors hover:text-text disabled:cursor-not-allowed disabled:opacity-40"
          >
            <ChevronLeft className="h-3.5 w-3.5" />
            上一步
          </button>

          <span className="ml-auto text-[10px] text-text-subtle">
            这些设置都可以在「设置」页随时修改
          </span>

          <button
            onClick={() =>
              isLast ? completeOnboarding() : setStep((value) => value + 1)
            }
            className="focus-ring flex cursor-pointer items-center gap-1.5 rounded-lg border border-primary/40 bg-primary/15 px-4 py-2 text-[12px] text-primary-3 transition-colors hover:bg-primary/25"
          >
            {isLast ? "开始使用" : "下一步"}
            {!isLast && <ChevronRight className="h-3.5 w-3.5" />}
          </button>
        </footer>
      </div>
    </div>
  );
}
