import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import EditorToolbar from "../EditorToolbar";
import type { ViewMode } from "../../../types";

function renderToolbar(overrides: Partial<Parameters<typeof EditorToolbar>[0]> = {}) {
  const props = {
    title: "笔记标题",
    viewMode: "split" as ViewMode,
    saving: false,
    savedAt: null as number | null,
    onTitleChange: vi.fn(),
    onViewModeChange: vi.fn(),
    ...overrides,
  };
  render(<EditorToolbar {...props} />);
  return props;
}

describe("EditorToolbar", () => {
  it("展示标题输入框并回传修改", () => {
    const props = renderToolbar();
    fireEvent.change(screen.getByPlaceholderText("笔记标题"), {
      target: { value: "新的标题" },
    });
    expect(props.onTitleChange).toHaveBeenCalledWith("新的标题");
  });

  it("三个视图模式按钮均可用并回传所选模式", () => {
    const props = renderToolbar();
    for (const label of ["源码", "分栏", "预览"]) {
      expect(screen.getByText(label)).toBeTruthy();
    }
    fireEvent.click(screen.getByText("源码"));
    expect(props.onViewModeChange).toHaveBeenCalledWith("source");
    fireEvent.click(screen.getByText("预览"));
    expect(props.onViewModeChange).toHaveBeenCalledWith("preview");
  });

  it("保存中显示进度文案", () => {
    renderToolbar({ saving: true });
    expect(screen.getByText("保存中…")).toBeTruthy();
  });

  it("已保存时显示保存时间", () => {
    renderToolbar({ savedAt: Date.now() });
    expect(screen.getByText(/已保存/)).toBeTruthy();
  });

  it("未保存且无历史时无状态文案", () => {
    renderToolbar({ saving: false, savedAt: null });
    expect(screen.queryByText(/已保存/)).toBeNull();
    expect(screen.queryByText("保存中…")).toBeNull();
  });
});
