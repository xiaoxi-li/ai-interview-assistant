import { describe, expect, it } from "vitest";
import {
  formatAccelerator,
  updateBinding,
  validateHotkeys,
} from "../hotkeys";
import type { HotkeyBinding } from "../../types";

function key(
  key: string,
  modifiers: Partial<{ ctrl: boolean; alt: boolean; shift: boolean; meta: boolean }> = {}
) {
  return {
    key,
    ctrlKey: modifiers.ctrl ?? false,
    altKey: modifiers.alt ?? false,
    shiftKey: modifiers.shift ?? false,
    metaKey: modifiers.meta ?? false,
  };
}

describe("formatAccelerator", () => {
  it("组合修饰键与主键", () => {
    expect(formatAccelerator(key("a", { ctrl: true, shift: true }))).toBe(
      "Ctrl+Shift+A"
    );
    expect(formatAccelerator(key("p", { alt: true }))).toBe("Alt+P");
    // 修饰键顺序固定为 Ctrl → Alt → Shift → Super，便于展示与比较
    expect(formatAccelerator(key("m", { meta: true, shift: true }))).toBe(
      "Shift+Super+M"
    );
  });

  it("仅有修饰键时返回 null", () => {
    expect(formatAccelerator(key("Control", { ctrl: true }))).toBeNull();
    expect(formatAccelerator(key("Shift", { shift: true }))).toBeNull();
  });

  it("无修饰键时返回 null", () => {
    expect(formatAccelerator(key("a"))).toBeNull();
    expect(formatAccelerator(key("F5"))).toBeNull();
  });

  it("功能键保留原始名称", () => {
    expect(formatAccelerator(key("F5", { ctrl: true }))).toBe("Ctrl+F5");
    expect(formatAccelerator(key("ArrowUp", { ctrl: true }))).toBe("Ctrl+ArrowUp");
  });
});

describe("validateHotkeys", () => {
  const base: HotkeyBinding[] = [
    { action: "ask", accelerator: "Ctrl+Shift+A" },
    { action: "copy_answer", accelerator: "Ctrl+Shift+C" },
  ];

  it("合法配置通过", () => {
    expect(validateHotkeys(base)).toBeNull();
  });

  it("空快捷键报错并带动作名", () => {
    expect(
      validateHotkeys([{ action: "ask", accelerator: "  " }])
    ).toContain("一键提问");
  });

  it("缺少修饰键报错", () => {
    expect(validateHotkeys([{ action: "ask", accelerator: "A" }])).toContain(
      "修饰键"
    );
  });

  it("重复快捷键报错（忽略大小写）", () => {
    const error = validateHotkeys([
      { action: "ask", accelerator: "Ctrl+A" },
      { action: "regenerate", accelerator: "ctrl+a" },
    ]);
    expect(error).toContain("冲突");
  });

  it("空列表视为合法", () => {
    expect(validateHotkeys([])).toBeNull();
  });
});

describe("updateBinding", () => {
  it("更新已有动作", () => {
    const next = updateBinding(
      [{ action: "ask", accelerator: "Ctrl+A" }],
      "ask",
      "Ctrl+B"
    );
    expect(next).toEqual([{ action: "ask", accelerator: "Ctrl+B" }]);
  });

  it("新增动作", () => {
    const next = updateBinding([], "ask", "Ctrl+A");
    expect(next).toEqual([{ action: "ask", accelerator: "Ctrl+A" }]);
  });

  it("不修改其他动作", () => {
    const next = updateBinding(
      [
        { action: "ask", accelerator: "Ctrl+A" },
        { action: "copy_answer", accelerator: "Ctrl+C" },
      ],
      "ask",
      "Ctrl+K"
    );
    expect(next[1]).toEqual({ action: "copy_answer", accelerator: "Ctrl+C" });
  });
});
