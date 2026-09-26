import { useEffect } from "react";
import { Settings as SettingsIcon } from "lucide-react";
import AiSettingsSection from "../components/settings/AiSettingsSection";
import AsrSettingsSection from "../components/settings/AsrSettingsSection";
import AudioSettingsSection from "../components/settings/AudioSettingsSection";
import { useSettingsStore } from "../stores/settingsStore";

const PLACEHOLDER_SECTIONS = [
  { title: "模型管理", desc: "嵌入 / VAD / 本地 ASR 模型的下载、更新与删除", task: "ST-10" },
  { title: "知识库", desc: "存储路径、向量库路径、默认嵌入模型", task: "ST-06" },
  { title: "界面", desc: "主题、快捷键、开机自启、托盘行为", task: "ST-07" },
  { title: "隐私与安全", desc: "音频上传开关、本地加密、清除数据", task: "ST-09" },
];

export default function SettingsPage() {
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
  const audioConfig = useSettingsStore((s) => s.audioConfig);
  const audioDevices = useSettingsStore((s) => s.audioDevices);
  const loadAudioSettings = useSettingsStore((s) => s.loadAudioSettings);
  const refreshAudioDevices = useSettingsStore((s) => s.refreshAudioDevices);
  const saveAudioConfig = useSettingsStore((s) => s.saveAudioConfig);

  const platform = aiConfig?.platform;

  useEffect(() => {
    loadAiSettings();
    loadAsrSettings();
    loadAudioSettings();
  }, [loadAiSettings, loadAsrSettings, loadAudioSettings]);

  useEffect(() => {
    if (platform) {
      loadApiKeyStatus(platform);
    }
  }, [platform, loadApiKeyStatus]);

  return (
    <div className="flex h-full flex-col">
      <div className="flex h-12 shrink-0 items-center gap-3 border-b border-white/5 px-4">
        <SettingsIcon className="h-4 w-4 text-primary-3" />
        <h1 className="text-[15px] font-semibold">设置</h1>
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
            <section className="glass rounded-xl p-4 text-[12px] text-text-subtle">
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

          <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
            {PLACEHOLDER_SECTIONS.map((section) => (
              <div
                key={section.title}
                className="glass rounded-xl p-4 opacity-80"
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
