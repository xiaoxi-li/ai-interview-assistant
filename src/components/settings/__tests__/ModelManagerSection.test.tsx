import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import ModelManagerSection from "../ModelManagerSection";
import type { ModelProgress, ModelStatus } from "../../../types";

function makeModel(overrides: Partial<ModelStatus> = {}): ModelStatus {
  return {
    id: "embedding-minilm-zh",
    name: "中文嵌入模型（MiniLM）",
    kind: "embedding",
    fileName: "embedding.onnx",
    path: "embedding/minilm-zh.onnx",
    sha256: "",
    sizeBytes: 94_371_840,
    description: "本地向量检索使用的轻量嵌入模型",
    requiredForLocalAsr: false,
    installed: false,
    resolvedPath: null,
    partialBytes: 0,
    ...overrides,
  };
}

function setup(
  models: ModelStatus[] = [makeModel()],
  progress: Record<string, ModelProgress> = {},
  downloadingId: string | null = null
) {
  const handlers = { onDownload: vi.fn(), onDelete: vi.fn() };
  render(
    <ModelManagerSection
      models={models}
      progress={progress}
      downloadingId={downloadingId}
      {...handlers}
    />
  );
  return handlers;
}

describe("ModelManagerSection", () => {
  it("展示模型名称与类别标签", () => {
    setup([
      makeModel(),
      makeModel({ id: "vad-silero", name: "Silero VAD", kind: "vad" }),
      makeModel({ id: "asr-1", name: "Paraformer 中文", kind: "asr" }),
    ]);

    expect(screen.getByText("中文嵌入模型（MiniLM）")).toBeTruthy();
    expect(screen.getByText("嵌入模型")).toBeTruthy();
    expect(screen.getByText("VAD")).toBeTruthy();
    // 类别标签与模型名区分，避免同名导致查询歧义
    expect(screen.getByText("本地 ASR")).toBeTruthy();
    expect(screen.getByText("Paraformer 中文")).toBeTruthy();
  });

  it("已安装模型显示标记与「重新下载」", () => {
    setup([makeModel({ installed: true, resolvedPath: "C:/models/x.onnx" })]);
    expect(screen.getByText("已安装")).toBeTruthy();
    expect(screen.getByText("重新下载")).toBeTruthy();
    expect(screen.getByText("C:/models/x.onnx")).toBeTruthy();
  });

  it("存在半成品时展示已缓存大小", () => {
    setup([makeModel({ partialBytes: 524_288 })]);
    expect(screen.getByText("已缓存 512 KB")).toBeTruthy();
  });

  it("未安装且无缓存时不显示删除按钮", () => {
    setup([makeModel()]);
    expect(screen.queryByTitle("删除本地文件")).toBeNull();
  });

  it("有文件时可删除并触发回调", () => {
    const h = setup([makeModel({ installed: true })]);
    fireEvent.click(screen.getByTitle("删除本地文件"));
    expect(h.onDelete).toHaveBeenCalledWith("embedding-minilm-zh");
  });

  it("点击下载触发回调", () => {
    const h = setup([makeModel()]);
    fireEvent.click(screen.getByText("下载"));
    expect(h.onDownload).toHaveBeenCalledWith("embedding-minilm-zh");
  });

  it("下载中展示进度与百分比并禁用按钮", () => {
    setup(
      [makeModel()],
      { "embedding-minilm-zh": { id: "embedding-minilm-zh", downloaded: 47_185_920, total: 94_371_840 } },
      "embedding-minilm-zh"
    );

    const button = screen.getByText("下载中…") as HTMLButtonElement;
    expect(button.disabled).toBe(true);
    expect(screen.getByText(/45\.0 MB \/ 90\.0 MB（50%）/)).toBeTruthy();
  });

  it("总大小未知时展示连接提示", () => {
    setup(
      [makeModel()],
      { "embedding-minilm-zh": { id: "embedding-minilm-zh", downloaded: 0, total: null } },
      "embedding-minilm-zh"
    );
    expect(screen.getByText("正在连接镜像源…")).toBeTruthy();
  });
});
