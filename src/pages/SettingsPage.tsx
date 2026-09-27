import { useEffect } from "react";
import { Languages, Settings as SettingsIcon } from "lucide-react";
import AiSettingsSection from "../components/settings/AiSettingsSection";
import AsrSettingsSection from "../components/settings/AsrSettingsSection";
import AudioSettingsSection from "../components/settings/AudioSettingsSection";
import InterfaceSettingsSection from "../components/settings/InterfaceSettingsSection";
import KnowledgeSettingsSection from "../components/settings/KnowledgeSettingsSection";
import ModelManagerSection from "../components/settings/ModelManagerSection";
import PrivacySettingsSection from "../components/settings/PrivacySettingsSection";
import UpdateSection from "../components/settings/UpdateSection";
import { useModelProgress } from "../hooks/useModelProgress";
import { useT } from "../hooks/useT";
import { LOCALES, type Locale } from "../lib/i18n";
import { useAppStore } from "../stores/appStore";
import { useSettingsStore } from "../stores/settingsStore";

const PLACEHOLDER_SECTIONS: { title: string; desc: string; task: string }[] = [];

export default function SettingsPage() {
  const locale = useAppStore((s) => s.locale);
  const setLocale = useAppStore((s) => s.setLocale);
  const t = useT();

  const aiConfig = useSettingsStore((s) => s.aiConfig);
  const presets = useSettingsStore((s) => s.presets);
  const apiKeys = useSettingsStore((s) => s.apiKeys);
  const loadAiSettings = useSettingsStore((s) => s.loadAiSettings);
  const saveAiConfig = useSettingsStore((s) => s.saveAiConfig);
  const loadApiKeyStatus = useSettingsStore((s) => s.loadApiKeyStatus);
  const saveApiKey = useSettingsStore((s) => s.saveApiKey);
  const deleteApiKey = useSettingsStore((s) => s.deleteApiKey);
  const testing = useSettingsStore((s) => s.testing);
  const testResult = useSettingsStore((s) => s.testResult);
  const testAiConnection = useSettingsStore((s) => s.testAiConnection);
  const asrConfig = useSettingsStore((s) => s.asrConfig);
  const asrProviders = useSettingsStore((s) => s.asrProviders);
  const asrCredentials = useSettingsStore((s) => s.asrCredentials);
  const loadAsrSettings = useSettingsStore((s) => s.loadAsrSettings);
  const saveAsrConfig = useSettingsStore((s) => s.saveAsrConfig);
  const loadAsrCredentialStatus = useSettingsStore((s) => s.loadAsrCredentialStatus);
  const saveAsrCredential = useSettingsStore((s) => s.saveAsrCredential);
  const deleteAsrCredentials = useSettingsStore((s) => s.deleteAsrCredentials);
  const testingAsr = useSettingsStore((s) => s.testingAsr);
  const asrTestResult = useSettingsStore((s) => s.asrTestResult);
  const testAsrConnection = useSettingsStore((s) => s.testAsrConnection);
  const audioConfig = useSettingsStore((s) => s.audioConfig);
  const audioDevices = useSettingsStore((s) => s.audioDevices);
  const loadAudioSettings = useSettingsStore((s) => s.loadAudioSettings);
  const refreshAudioDevices = useSettingsStore((s) => s.refreshAudioDevices);
  const saveAudioConfig = useSettingsStore((s) => s.saveAudioConfig);
  const privacyConfig = useSettingsStore((s) => s.privacyConfig);
  const clearing = useSettingsStore((s) => s.clearing);
  const loadPrivacySettings = useSettingsStore((s) => s.loadPrivacySettings);
  const savePrivacyConfig = useSettingsStore((s) => s.savePrivacyConfig);
  const clearLocalData = useSettingsStore((s) => s.clearLocalData);
  const models = useSettingsStore((s) => s.models);
  const modelProgress = useSettingsStore((s) => s.modelProgress);
  const downloadingId = useSettingsStore((s) => s.downloadingId);
  const loadModels = useSettingsStore((s) => s.loadModels);
  const downloadModel = useSettingsStore((s) => s.downloadModel);
  const deleteModel = useSettingsStore((s) => s.deleteModel);
  const applyModelProgress = useSettingsStore((s) => s.applyModelProgress);
  const modelErrors = useSettingsStore((s) => s.modelErrors);
  const knowledgeConfig = useSettingsStore((s) => s.knowledgeConfig);
  const storagePaths = useSettingsStore((s) => s.storagePaths);
  const loadKnowledgeSettings = useSettingsStore((s) => s.loadKnowledgeSettings);
  const saveKnowledgeConfig = useSettingsStore((s) => s.saveKnowledgeConfig);
  const interfaceConfig = useSettingsStore((s) => s.interfaceConfig);
  const autoStartEnabled = useSettingsStore((s) => s.autoStartEnabled);
  const loadInterfaceSettings = useSettingsStore((s) => s.loadInterfaceSettings);
  const saveInterfaceConfig = useSettingsStore((s) => s.saveInterfaceConfig);
  const setAutoStart = useSettingsStore((s) => s.setAutoStart);

  // 订阅模型下载进度事件
  useModelProgress(applyModelProgress);

  const platform = aiConfig?.platform;

  useEffect(() => {
    loadAiSettings();
    loadAsrSettings();
    loadAudioSettings();
    loadPrivacySettings();
    loadModels();
    loadKnowledgeSettings();
    loadInterfaceSettings();
  }, [
    loadAiSettings,
    loadAsrSettings,
    loadAudioSettings,
    loadPrivacySettings,
    loadModels,
    loadKnowledgeSettings,
    loadInterfaceSettings,
  ]);

  useEffect(() => {
    if (platform) {
      loadApiKeyStatus(platform);
    }
  }, [platform, loadApiKeyStatus]);

  return (
    <div className="flex h-full flex-col">
      <div className="flex h-12 shrink-0 items-center gap-3 border-b border-white/[0.06] px-4">
        <SettingsIcon className="h-4 w-4 text-primary-3" />
        <h1 className="text-[15px] font-semibold">{t("设置")}</h1>

        {/* 界面语言（i18n）：中文原文为键，英文按需翻译 */}
        <label className="ml-auto flex items-center gap-1.5 text-[11px] text-text-subtle">
          <Languages className="h-3.5 w-3.5" />
          {t("界面语言")}
          <select
            value={locale}
            onChange={(event) => setLocale(event.target.value as Locale)}
            className="focus-ring cursor-pointer rounded-lg border border-white/10 bg-background-3/60 px-2 py-1 text-[11px] text-text outline-none transition-colors focus:border-primary/50"
          >
            {LOCALES.map((item) => (
              <option key={item.value} value={item.value}>
                {item.label}
              </option>
            ))}
          </select>
        </label>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto p-4">
        <div className="mx-auto flex max-w-4xl flex-col gap-3">
          {aiConfig ? (
            <AiSettingsSection
              config={aiConfig}
              presets={presets}
              apiKeyStatus={platform ? apiKeys[platform] : undefined}
              onSaveConfig={saveAiConfig}
              onSaveApiKey={saveApiKey}
              onDeleteApiKey={deleteApiKey}
              onTestConnection={testAiConnection}
              testing={testing}
              testResult={testResult}
            />
          ) : (
            <section className="glass rounded-xl p-4 text-[12px] text-text-subtle shadow-card">
              正在加载 AI 配置…
            </section>
          )}

          {asrConfig && (
            <AsrSettingsSection
              config={asrConfig}
              providers={asrProviders}
              credentialStatus={asrCredentials[asrConfig.cloudProvider]}
              onSaveConfig={saveAsrConfig}
              onSaveCredential={saveAsrCredential}
              onDeleteCredentials={deleteAsrCredentials}
              onProviderChange={loadAsrCredentialStatus}
              onTestConnection={testAsrConnection}
              testing={testingAsr}
              testResult={asrTestResult}
            />
          )}

          {audioConfig && (
            <AudioSettingsSection
              config={audioConfig}
              devices={audioDevices}
              onRefresh={refreshAudioDevices}
              onSave={saveAudioConfig}
            />
          )}

          {knowledgeConfig && (
            <KnowledgeSettingsSection
              config={knowledgeConfig}
              storagePaths={storagePaths}
              models={models}
              onSave={saveKnowledgeConfig}
            />
          )}

          {models.length > 0 && (
            <ModelManagerSection
              models={models}
              progress={modelProgress}
              downloadingId={downloadingId}
              onDownload={downloadModel}
              onDelete={deleteModel}
              errors={modelErrors}
            />
          )}

          {privacyConfig && (
            <PrivacySettingsSection
              config={privacyConfig}
              cloudAsrEnabled={asrConfig?.mode === "cloud"}
              clearing={clearing}
              onSave={savePrivacyConfig}
              onClearData={clearLocalData}
            />
          )}

          {interfaceConfig && (
            <InterfaceSettingsSection
              config={interfaceConfig}
              autoStartEnabled={autoStartEnabled}
              onSave={saveInterfaceConfig}
              onToggleAutoStart={setAutoStart}
            />
          )}

          {/* 关于与更新（P2） */}
          <UpdateSection />

          <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
            {PLACEHOLDER_SECTIONS.map((section) => (
              <div
                key={section.title}
                className="glass rounded-xl p-4 opacity-80 shadow-card"
              >
                <div className="flex items-center gap-2">
                  <h3 className="text-[13px] font-medium text-text">
                    {section.title}
                  </h3>
                  <span className="rounded bg-white/[0.06] px-1.5 py-0.5 text-[10px] text-text-subtle">
                    {section.task}
                  </span>
                </div>
                <p className="mt-1 text-[12px] leading-relaxed text-text-muted">
                  {section.desc}
                </p>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
