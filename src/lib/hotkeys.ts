import type { HotkeyAction, HotkeyBinding } from "../types";

/** 快捷键动作元信息（与后端 HOTKEY_ACTIONS 对应） */
export const HOTKEY_ACTIONS: { action: HotkeyAction; label: string }[] = [
  { action: "ask", label: "一键提问" },
  { action: "regenerate", label: "重新生成回答" },
  { action: "copy_answer", label: "复制回答" },
  { action: "pause_capture", label: "暂停/恢复捕获" },
  { action: "toggle_mini", label: "切换迷你模式" },
];

/** 需要修饰键的关键字面量（单独按下不构成快捷键） */
const MODIFIER_KEYS = ["Control", "Alt", "Shift", "Meta"];

/**
 * 将键盘事件转换为加速键字符串（如 `Ctrl+Shift+A`）。
 * 返回 null 表示当前按键不构成有效快捷键（仅有修饰键或无修饰键）。
 */
export function formatAccelerator(
  event: Pick<
    KeyboardEvent,
    "key" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey"
  >
): string | null {
  if (MODIFIER_KEYS.includes(event.key)) return null;

  const parts: string[] = [];
  if (event.ctrlKey) parts.push("Ctrl");
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");
  if (event.metaKey) parts.push("Super");

  // 必须至少包含一个修饰键，避免与普通输入冲突
  if (parts.length === 0) return null;

  const key = event.key.length === 1 ? event.key.toUpperCase() : event.key;
  parts.push(key);
  return parts.join("+");
}

/** 校验快捷键集合：非空、含修饰键且不重复 */
export function validateHotkeys(hotkeys: HotkeyBinding[]): string | null {
  const seen = new Set<string>();

  for (const binding of hotkeys) {
    const accelerator = binding.accelerator.trim();
    const label =
      HOTKEY_ACTIONS.find((item) => item.action === binding.action)?.label ??
      binding.action;

    if (!accelerator) return `${label} 的快捷键不能为空`;
    if (!accelerator.includes("+")) {
      return `${label} 的快捷键需包含修饰键（如 Ctrl+Shift+A）`;
    }

    const normalized = accelerator.toLowerCase();
    if (seen.has(normalized)) return `快捷键冲突：${accelerator}`;
    seen.add(normalized);
  }
  return null;
}

/** 按下某个动作的加速键时更新绑定列表 */
export function updateBinding(
  hotkeys: HotkeyBinding[],
  action: string,
  accelerator: string
): HotkeyBinding[] {
  const exists = hotkeys.some((binding) => binding.action === action);
  if (!exists) {
    return [...hotkeys, { action, accelerator }];
  }
  return hotkeys.map((binding) =>
    binding.action === action ? { ...binding, accelerator } : binding
  );
}
