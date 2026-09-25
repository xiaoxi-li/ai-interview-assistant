import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import MarkdownView from "../MarkdownView";

const SAMPLE = `# 项目架构

| 模块 | 职责 |
|---|---|
| 检索 | 全文与向量召回 |

- [x] 已完成任务
- [ ] 未完成任务

\`\`\`ts
const dim: number = 384;
\`\`\`

![架构示意](https://example.com/a.png)
`;

describe("MarkdownView", () => {
  it("渲染一级标题", () => {
    render(<MarkdownView content="# 一级标题" />);
    expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("一级标题");
  });

  it("渲染 GFM 表格", () => {
    render(<MarkdownView content={SAMPLE} />);
    expect(screen.getByRole("table")).toBeTruthy();
    expect(screen.getByText("检索")).toBeTruthy();
  });

  it("渲染任务列表复选框及勾选状态", () => {
    const { container } = render(<MarkdownView content={SAMPLE} />);
    const boxes = container.querySelectorAll<HTMLInputElement>(
      'input[type="checkbox"]'
    );
    expect(boxes.length).toBe(2);
    expect(boxes[0].checked).toBe(true);
    expect(boxes[1].checked).toBe(false);
  });

  it("渲染代码块并标注语言", () => {
    const { container } = render(<MarkdownView content={SAMPLE} />);
    const code = container.querySelector("pre code");
    expect(code).toBeTruthy();
    expect(code?.className).toContain("language-ts");
  });

  it("渲染图片及其替代文本", () => {
    render(<MarkdownView content={SAMPLE} />);
    expect(screen.getByAltText("架构示意")).toBeTruthy();
  });

  it("空内容不抛错", () => {
    const { container } = render(<MarkdownView content="" />);
    expect(container.querySelector(".markdown-body")).toBeTruthy();
  });
});
