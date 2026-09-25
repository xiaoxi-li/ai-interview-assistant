import { Settings as SettingsIcon } from "lucide-react";

const SECTIONS = [
  { title: "AI 平台", desc: "DeepSeek / Qwen / GLM / ChatGPT 配置与连接测试" },
  { title: "语音识别（ASR）", desc: "云端/本地模式、讯飞服务商、模型、语言、VAD 阈值" },
  { title: "音频设备", desc: "麦克风与系统音频输出设备选择" },
  { title: "模型管理", desc: "本地模型下载、更新与删除" },
  { title: "隐私与安全", desc: "音频上传开关、本地加密、清除数据" },
  { title: "界面", desc: "主题、快捷键、开机自启、托盘行为" },
];

export default function SettingsPage() {
  return (
    <div className="flex h-full flex-col">
      <div className="flex h-12 shrink-0 items-center gap-3 border-b border-white/5 px-4">
        <SettingsIcon className="h-4 w-4 text-primary-3" />
        <h1 className="text-[15px] font-semibold">设置</h1>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto p-4">
        <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
          {SECTIONS.map((s) => (
            <div
              key={s.title}
              className="glass cursor-pointer rounded-xl p-4 transition-colors hover:border-primary/30"
            >
              <h3 className="text-[13px] font-medium text-text">{s.title}</h3>
              <p className="mt-1 text-[12px] leading-relaxed text-text-muted">
                {s.desc}
              </p>
            </div>
          ))}
        </div>
        <p className="mt-4 px-1 text-[11px] text-text-subtle">
          设置分组表单将在 ST-01 ~ ST-10 任务中逐项实现
        </p>
      </div>
    </div>
  );
}
