import { useEffect, useMemo, useState } from "react";
import { AlertCircle, Monitor, Moon, Power, Save, Sun } from "lucide-react";
import { cn } from "../../lib/utils";
import { SaveFeedback } from "./SaveFeedback";
import { HOTKEY_ACTIONS, updateBinding, validateHotkeys } from "../../lib/hotkeys";
import HotkeyInput from "./HotkeyInput";
import type { InterfaceConfig } from "../../types";

const LABEL = "mb-1.5 block text-[11px] text-text-muted";

interface InterfaceSettingsSectionProps {
  config: InterfaceConfig;
  autoStartEnabled: boolean;
  onSave: (config: InterfaceConfig) => void;
  onToggleAutoStart: (enabled: boolean) => void;
}

export default function InterfaceSettingsSection({
  config,
  autoStartEnabled,
  onSave,
  onToggleAutoStart,
}: InterfaceSettingsSectionProps) {
  const [draft, setDraft] = useState<InterfaceConfig>(config);

  useEffect(() => {
    setDraft(config);
  }, [config]);

  const hotkeyError = useMemo(() => validateHotkeys(draft.hotkeys), [draft.hotkeys]);

  const acceleratorFor = (action: string) =>
    draft.hotkeys.find((binding) => binding.action === action)?.accelerator ?? "";

  return (
    <section className="glass rounded-xl p-4 shadow-card">
      <header className="mb-4 flex items-center gap-2">
        <h2 className="text-[13px] font-semibold">界面与快捷键</h2>
        <span className="text-[11px] text-text-subtle">
          全局快捷键由原生层注册，面试中无需切换窗口
        </span>
      </header>

      {/* 主题 */}
      <div className="mb-4">
        <span className={LABEL}>主题</span>
        <div className="flex gap-1.5">
          {(
            [
              { theme: "dark" as const, label: "深色", icon: Moon },
              { theme: "light" as const, label: "浅色", icon: Sun },
            ]
          ).map(({ theme, label, icon: Icon }) => (
            <button
              key={theme}
              onClick={() => setDraft({ ...draft, theme })}
              className={cn(
                "focus-ring flex cursor-pointer items-center gap-1.5 rounded-lg border px-3 py-1.5 text-[12px] transition-colors",
                draft.theme === theme
                  ? "border-primary/40 bg-primary/15 text-primary-3"
                  : "border-white/10 bg-background-3/60 text-text-muted hover:text-text"
              )}
            >
              <Icon className="h-3.5 w-3.5" />
              {label}
            </button>
          ))}
        </div>
      </div>

      {/* 快捷键 */}
      <div className="mb-4">
        <span className={LABEL}>全局快捷键</span>
        <div className="flex flex-col gap-2">
          {HOTKEY_ACTIONS.map(({ action, label }) => (
            <div key={action} className="flex items-center gap-3">
              <span className="w-28 shrink-0 text-[12px] text-text-muted">
                {label}
              </span>
              <HotkeyInput
                id={`hotkey-${action}`}
                value={acceleratorFor(action)}
                invalid={Boolean(hotkeyError)}
                onChange={(accelerator) =>
                  setDraft({
                    ...draft,
                    hotkeys: updateBinding(draft.hotkeys, action, accelerator),
                  })
                }
              />
            </div>
          ))}
        </div>
        <p className="mt-1.5 text-[10px] text-text-subtle">
          点击输入框后按下组合键即可录制，Backspace 清空，Esc 取消
        </p>
      </div>

      {/* 启动与托盘 */}
      <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
        <label className="flex cursor-pointer items-start gap-3 rounded-md border border-white/[0.06] bg-background-3/40 p-3">
          <Power className="mt-0.5 h-4 w-4 shrink-0 text-text-muted" />
          <span className="min-w-0 flex-1">
            <span className="block text-[12px] font-medium text-text">开机自启</span>
            <span className="mt-0.5 block text-[11px] text-text-muted">
              登录系统后自动启动并驻留托盘
            </span>
          </span>
          <input
            id="interface-autostart"
            aria-label="开机自启"
            type="checkbox"
            checked={autoStartEnabled}
            onChange={(e) => onToggleAutoStart(e.target.checked)}
            className="focus-ring mt-0.5 h-4 w-4 shrink-0 cursor-pointer rounded accent-[#4F6BED]"
          />
        </label>

        <div>
          <label className={LABEL} htmlFor="interface-tray">
            <span className="inline-flex items-center gap-1.5">
              <Monitor className="h-3 w-3" />
              关闭窗口行为
            </span>
          </label>
          <select
            id="interface-tray"
            value={draft.trayBehavior}
            onChange={(e) =>
              setDraft({
                ...draft,
                trayBehavior: e.target.value as InterfaceConfig["trayBehavior"],
              })
            }
            className="focus-ring w-full cursor-pointer rounded-lg border border-white/10 bg-background-3/60 px-3 py-2 text-[13px] text-text outline-none focus:border-primary/50"
          >
            <option value="minimize">最小化到系统托盘（保留捕获）</option>
            <option value="quit">直接退出应用</option>
          </select>
        </div>
      </div>

      <div className="mt-4 flex items-center gap-3">
        {hotkeyError && (
          <span className="flex items-center gap-1.5 text-[11px] text-danger">
            <AlertCircle className="h-3 w-3" />
            {hotkeyError}
          </span>
        )}
        <button
          onClick={() => onSave(draft)}
          disabled={Boolean(hotkeyError)}
          className="focus-ring ml-auto flex cursor-pointer items-center gap-1.5 rounded-lg border border-primary/40 bg-primary/15 px-4 py-2 text-[12px] text-primary-3 transition-colors hover:bg-primary/25 disabled:cursor-not-allowed disabled:opacity-40"
        >
          <Save className="h-3.5 w-3.5" />
          保存配置
        </button>
        {/* 保存反馈：覆盖「保存配置」「开机自启开关」 */}
        <SaveFeedback target="interface" />
      </div>
    </section>
  );
}
