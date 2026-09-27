import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import ModelManagerSection from "../ModelManagerSection";
import type { ModelProgress, ModelStatus } from "../../../types";

function makeModel(overrides: Partial<ModelStatus> = {}): ModelStatus {
  return {
    id: "bge-m3",
    name: "嵌入模型（BGE-M3）",
    kind: "embedding",
    repo: "BAAI/bge-m3",
    files: [
      {
        remotePath: "onnx/model.onnx",
        localName: "model.onnx",
        sha256: "",
        sizeBytes: 724_923,
      },
    ],
    description: "本地向量检索使用的多语言嵌入模型",
    requiredForLocalAsr: false,
    installed: false,
    resolvedPath: null,
    partialBytes: 0,
    ...overrides,
  };
}

type ExtraProps = Partial<
  Omit<
    Parameters<typeof ModelManagerSection>[0],
    "models" | "progress" | "downloadingId" | "onDownload" | "onDelete"
  >
>;

function setup(
  models: ModelStatus[] = [makeModel()],
  progress: Record<string, ModelProgress> = {},
  downloadingId: string | null = null,
  extra: ExtraProps = {}
) {
  const handlers = { onDownload: vi.fn(), onDelete: vi.fn() };
  render(
    <ModelManagerSection
      models={models}
      progress={progress}
      downloadingId={downloadingId}
      {...handlers}
      {...extra}
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

    expect(screen.getByText("嵌入模型（BGE-M3）")).toBeTruthy();
    expect(screen.getByText("嵌入模型")).toBeTruthy();
    expect(screen.getByText("VAD")).toBeTruthy();
    // 类别标签与模型名区分，避免同名导致查询歧义
    expect(screen.getByText("本地 ASR")).toBeTruthy();
    expect(screen.getByText("Paraformer 中文")).toBeTruthy();
  });

  it("已安装模型显示标记与「重新下载」", () => {
    setup([makeModel({ installed: true, resolvedPath: "C:/models/bge-m3" })]);
    expect(screen.getByText("已安装")).toBeTruthy();
    expect(screen.getByText("重新下载")).toBeTruthy();
    expect(screen.getByText("C:/models/bge-m3")).toBeTruthy();
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
    expect(h.onDelete).toHaveBeenCalledWith("bge-m3");
  });

  it("点击下载触发回调", () => {
    const h = setup([makeModel()]);
    fireEvent.click(screen.getByText("下载"));
    expect(h.onDownload).toHaveBeenCalledWith("bge-m3");
  });

  it("下载中展示进度与百分比并禁用按钮", () => {
    setup(
      [makeModel()],
      { "bge-m3": { id: "bge-m3", downloaded: 47_185_920, total: 94_371_840 } },
      "bge-m3"
    );

    const button = screen.getByText("下载中…") as HTMLButtonElement;
    expect(button.disabled).toBe(true);
    expect(screen.getByText(/45\.0 MB \/ 90\.0 MB（50%）/)).toBeTruthy();
  });

  it("总大小未知时展示连接提示", () => {
    setup(
      [makeModel()],
      { "bge-m3": { id: "bge-m3", downloaded: 0, total: null } },
      "bge-m3"
    );
    expect(screen.getByText("正在连接下载源…")).toBeTruthy();
  });

  it("下载失败时内联展示错误（不再静默消失）", () => {
    setup([makeModel()], {}, null, {
      errors: { "bge-m3": "下载失败：网络错误" },
    });
    expect(screen.getByText(/下载失败：网络错误/)).toBeTruthy();
  });
});
