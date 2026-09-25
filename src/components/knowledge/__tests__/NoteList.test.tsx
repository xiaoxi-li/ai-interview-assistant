import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import NoteList from "../NoteList";
import type { Note } from "../../../types";

function makeNote(overrides: Partial<Note> = {}): Note {
  return {
    id: "n1",
    title: "面试笔记",
    content: "内容正文",
    groupId: null,
    tags: [],
    isDeleted: false,
    createdAt: 0,
    updatedAt: Date.now(),
    ...overrides,
  };
}

function setup(props: Partial<Parameters<typeof NoteList>[0]> = {}) {
  const handlers = {
    onSelect: vi.fn(),
    onTrash: vi.fn(),
    onRestore: vi.fn(),
    onPurge: vi.fn(),
  };
  render(
    <NoteList
      notes={[makeNote()]}
      selectedId={null}
      keyword=""
      mode="all"
      {...handlers}
      {...props}
    />
  );
  return handlers;
}

describe("NoteList", () => {
  it("全部模式下点击条目触发选中", () => {
    const h = setup();
    fireEvent.click(screen.getByText("面试笔记"));
    expect(h.onSelect).toHaveBeenCalledWith("n1");
  });

  it("全部模式下点击删除触发移入回收站且不触发选中", () => {
    const h = setup();
    fireEvent.click(screen.getByTitle("移入回收站"));
    expect(h.onTrash).toHaveBeenCalledWith("n1");
    expect(h.onSelect).not.toHaveBeenCalled();
  });

  it("回收站模式提供恢复与彻底删除", () => {
    const h = setup({ mode: "trash" });
    expect(screen.queryByTitle("移入回收站")).toBeNull();

    fireEvent.click(screen.getByTitle("恢复"));
    expect(h.onRestore).toHaveBeenCalledWith("n1");

    fireEvent.click(screen.getByTitle("彻底删除"));
    expect(h.onPurge).toHaveBeenCalledWith("n1");
  });

  it("空列表文案区分为回收站为空与暂无笔记", () => {
    const { unmount } = render(
      <NoteList
        notes={[]}
        selectedId={null}
        keyword=""
        mode="trash"
        onSelect={vi.fn()}
        onTrash={vi.fn()}
        onRestore={vi.fn()}
        onPurge={vi.fn()}
      />
    );
    expect(screen.getByText("回收站为空")).toBeTruthy();
    unmount();

    render(
      <NoteList
        notes={[]}
        selectedId={null}
        keyword=""
        mode="all"
        onSelect={vi.fn()}
        onTrash={vi.fn()}
        onRestore={vi.fn()}
        onPurge={vi.fn()}
      />
    );
    expect(screen.getByText(/暂无笔记/)).toBeTruthy();
  });

  it("关键词无匹配时给出提示", () => {
    setup({ keyword: "不存在的关键词" });
    expect(screen.getByText("没有匹配的笔记")).toBeTruthy();
  });

  it("关键词命中正文时保留条目", () => {
    setup({ keyword: "内容正文" });
    expect(screen.getByText("面试笔记")).toBeTruthy();
  });
});
