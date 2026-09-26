import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import VersionPanel from "../VersionPanel";
import type { NoteVersion } from "../../../types";

function makeVersion(version: number, content: string): NoteVersion {
  return {
    id: `v${version}`,
    noteId: "n1",
    title: "标题",
    content,
    version,
    createdAt: Date.now(),
  };
}

function setup(versions: NoteVersion[] = [makeVersion(1, "第一版内容")]) {
  const handlers = { onRestore: vi.fn(), onClose: vi.fn() };
  render(<VersionPanel versions={versions} {...handlers} />);
  return handlers;
}

describe("VersionPanel", () => {
  it("空列表提示暂无历史版本", () => {
    setup([]);
    expect(screen.getByText(/暂无历史版本/)).toBeTruthy();
  });

  it("展示版本号与内容摘要", () => {
    setup([makeVersion(3, "第三版内容"), makeVersion(2, "第二版内容")]);
    expect(screen.getByText("v3")).toBeTruthy();
    expect(screen.getByText("v2")).toBeTruthy();
    expect(screen.getByText("第三版内容")).toBeTruthy();
  });

  it("计数徽标反映版本数量", () => {
    setup([makeVersion(1, "a"), makeVersion(2, "b")]);
    expect(screen.getByText("2")).toBeTruthy();
  });

  it("点击回滚触发回调并带上版本 ID", () => {
    const h = setup([makeVersion(2, "第二版")]);
    fireEvent.click(screen.getByTitle("回滚到 v2"));
    expect(h.onRestore).toHaveBeenCalledWith("v2");
  });

  it("点击关闭触发回调", () => {
    const h = setup();
    fireEvent.click(screen.getByTitle("关闭版本历史"));
    expect(h.onClose).toHaveBeenCalled();
  });
});
