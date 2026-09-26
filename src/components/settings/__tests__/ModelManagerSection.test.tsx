import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import ModelManagerSection from "../ModelManagerSection";
import type { MirrorCheckResult, ModelProgress, ModelStatus } from "../../../types";

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

  it("未配置镜像源时显示未配置徽标", () => {
    setup([makeModel()], {}, null, { mirror: "" });
    expect(screen.getByText("未配置")).toBeTruthy();
    expect(screen.getByPlaceholderText("https://your-mirror.example.com/ai-interview")).toBeTruthy();
  });

  it("已配置镜像源时显示已配置徽标", () => {
    setup([makeModel()], {}, null, { mirror: "https://mirror.example.com/ai" });
    expect(screen.getByText("已配置")).toBeTruthy();
  });

  it("保存镜像源触发回调并去空格", () => {
    const onSaveMirror = vi.fn();
    setup([makeModel()], {}, null, { onSaveMirror });

    const input = screen.getByPlaceholderText(
      "https://your-mirror.example.com/ai-interview"
    ) as HTMLInputElement;
    fireEvent.change(input, { target: { value: "  https://m.example.com/ai  " } });
    fireEvent.click(screen.getByText("保存"));

    expect(onSaveMirror).toHaveBeenCalledWith("https://m.example.com/ai");
  });

  it("测试连通性触发回调", () => {
    const onCheckMirror = vi.fn();
    setup([makeModel()], {}, null, { onCheckMirror });
    fireEvent.click(screen.getByText("测试连通性"));
    expect(onCheckMirror).toHaveBeenCalledTimes(1);
  });

  it("展示镜像源探测结果（含失败原因）", () => {
    const mirrorCheck: MirrorCheckResult = {
      mirror: "https://mirror.example.com/ai",
      configured: true,
      probes: [
        {
          id: "embedding-minilm-zh",
          name: "中文嵌入模型（MiniLM）",
          ok: false,
          message: "不可达：连接被拒绝",
          contentLength: null,
          supportsRange: false,
        },
      ],
    };
    setup([makeModel()], {}, null, { mirrorCheck });
    expect(screen.getByText(/不可达：连接被拒绝/)).toBeTruthy();
  });

  it("下载失败时内联展示错误（不再静默消失）", () => {
    setup([makeModel()], {}, null, {
      errors: { "embedding-minilm-zh": "模型镜像源未配置。请到「设置 → 知识库」填写镜像源地址。" },
    });
    expect(
      screen.getByText(/模型镜像源未配置。请到「设置 → 知识库」填写镜像源地址。/)
    ).toBeTruthy();
  });
});
