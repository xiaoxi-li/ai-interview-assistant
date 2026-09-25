import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import TagEditor from "../TagEditor";

function setup(tags: string[] = ["架构"]) {
  const handlers = { onAdd: vi.fn(), onRemove: vi.fn() };
  render(
    <TagEditor
      tags={tags}
      suggestions={["架构", "性能优化", "Rust"]}
      {...handlers}
    />
  );
  return handlers;
}

describe("TagEditor", () => {
  it("展示已有标签", () => {
    setup(["架构", "Rust"]);
    expect(screen.getByText("架构")).toBeTruthy();
    expect(screen.getByText("Rust")).toBeTruthy();
  });

  it("回车添加新标签并清空输入", () => {
    const h = setup();
    const input = screen.getByPlaceholderText("添加标签…") as HTMLInputElement;

    fireEvent.change(input, { target: { value: "性能优化" } });
    fireEvent.keyDown(input, { key: "Enter" });

    expect(h.onAdd).toHaveBeenCalledWith("性能优化");
    expect(input.value).toBe("");
  });

  it("重复标签不重复添加", () => {
    const h = setup(["架构"]);
    const input = screen.getByPlaceholderText("添加标签…");

    fireEvent.change(input, { target: { value: "架构" } });
    fireEvent.keyDown(input, { key: "Enter" });

    expect(h.onAdd).not.toHaveBeenCalled();
  });

  it("空白输入不触发添加", () => {
    const h = setup();
    const input = screen.getByPlaceholderText("添加标签…");

    fireEvent.change(input, { target: { value: "   " } });
    fireEvent.keyDown(input, { key: "Enter" });

    expect(h.onAdd).not.toHaveBeenCalled();
  });

  it("点击 × 移除标签", () => {
    const h = setup(["架构", "Rust"]);
    fireEvent.click(screen.getByTitle("移除标签 架构"));
    expect(h.onRemove).toHaveBeenCalledWith("架构");
  });

  it("Esc 清空草稿且不添加", () => {
    const h = setup();
    const input = screen.getByPlaceholderText("添加标签…") as HTMLInputElement;

    fireEvent.change(input, { target: { value: "临时" } });
    fireEvent.keyDown(input, { key: "Escape" });

    expect(input.value).toBe("");
    expect(h.onAdd).not.toHaveBeenCalled();
  });
});
