import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import GroupTree from "../GroupTree";
import { GROUP_DRAG_TYPE, NOTE_DRAG_TYPE } from "../../../lib/dnd";
import type { Group } from "../../../types";

const groups: Group[] = [
  { id: "g1", name: "前端工程", parentId: null, order: 0 },
  { id: "g2", name: "React", parentId: "g1", order: 0 },
  { id: "g3", name: "后端", parentId: null, order: 1 },
];

function setup(overrides: Partial<Parameters<typeof GroupTree>[0]> = {}) {
  const handlers = {
    onSelect: vi.fn(),
    onCreate: vi.fn(),
    onRename: vi.fn(),
    onDelete: vi.fn(),
    onMove: vi.fn(),
    onReorder: vi.fn(),
    onDropNote: vi.fn(),
  };
  render(
    <GroupTree
      groups={groups}
      selectedGroupId={undefined}
      noteCounts={{ g1: 2 }}
      totalCount={5}
      ungroupedCount={1}
      {...handlers}
      {...overrides}
    />
  );
  return handlers;
}

describe("GroupTree", () => {
  it("展示全部/未分组计数与群组节点", () => {
    setup();
    expect(screen.getByText("全部笔记")).toBeTruthy();
    expect(screen.getByText("未分组")).toBeTruthy();
    expect(screen.getByText("前端工程")).toBeTruthy();
    expect(screen.getByText("后端")).toBeTruthy();
  });

  it("点击群组触发选中", () => {
    const h = setup();
    fireEvent.click(screen.getByText("前端工程"));
    expect(h.onSelect).toHaveBeenCalledWith("g1");
  });

  it("展开后渲染子群组", () => {
    const h = setup();
    expect(screen.queryByText("React")).toBeNull();
    fireEvent.click(screen.getByTitle("展开"));
    expect(screen.getByText("React")).toBeTruthy();
    fireEvent.click(screen.getByText("React"));
    expect(h.onSelect).toHaveBeenCalledWith("g2");
  });

  it("新建子群组：输入名称回车后回调", () => {
    const h = setup();
    fireEvent.click(screen.getAllByTitle("新建子群组")[0]);
    const input = screen.getByPlaceholderText("群组名称");
    fireEvent.change(input, { target: { value: "Hooks" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(h.onCreate).toHaveBeenCalledWith("Hooks", "g1");
  });

  it("重命名群组：回车后回调新名称", () => {
    const h = setup();
    fireEvent.click(screen.getAllByTitle("重命名")[0]);
    const input = screen.getByDisplayValue("前端工程");
    fireEvent.change(input, { target: { value: "前端" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(h.onRename).toHaveBeenCalledWith("g1", "前端");
  });

  it("删除群组触发回调", () => {
    const h = setup();
    fireEvent.click(screen.getAllByTitle("删除群组")[1]);
    expect(h.onDelete).toHaveBeenCalledWith("g3");
  });

  it("拖拽笔记到群组触发归类", () => {
    const h = setup();
    const dataTransfer = {
      getData: (type: string) => (type === NOTE_DRAG_TYPE ? "n1" : ""),
    };
    fireEvent.drop(screen.getByText("后端"), { dataTransfer });
    expect(h.onDropNote).toHaveBeenCalledWith("n1", "g3");
  });

  it("同级拖拽触发重排", () => {
    const h = setup();
    const dataTransfer = {
      getData: (type: string) => (type === GROUP_DRAG_TYPE ? "g3" : ""),
    };
    fireEvent.drop(screen.getByText("前端工程"), { dataTransfer });
    expect(h.onReorder).toHaveBeenCalledWith(["g3", "g1"]);
    expect(h.onMove).not.toHaveBeenCalled();
  });

  it("拖拽到根级区域触发移出父级", () => {
    const h = setup();
    const dataTransfer = {
      getData: (type: string) => (type === GROUP_DRAG_TYPE ? "g2" : ""),
    };
    fireEvent.drop(screen.getByText("拖到此处移到根级"), { dataTransfer });
    expect(h.onMove).toHaveBeenCalledWith("g2", null);
  });
});
