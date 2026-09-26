import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import KnowledgeSettingsSection from "../KnowledgeSettingsSection";
import type { KnowledgeConfig, ModelStatus, StoragePaths } from "../../../types";

const storagePaths: StoragePaths = {
  dbPath: "C:/data/ai-interview.db",
  modelsDir: "C:/data/models",
};

function makeModel(id: string, kind: ModelStatus["kind"], installed: boolean): ModelStatus {
  return {
    id,
    name: id,
    kind,
    fileName: `${id}.bin`,
    path: `${id}.bin`,
    sha256: "",
    sizeBytes: 0,
    description: "",
    requiredForLocalAsr: false,
    installed,
    resolvedPath: null,
    partialBytes: 0,
  };
}

const models: ModelStatus[] = [
  makeModel("embedding-minilm-zh", "embedding", true),
  makeModel("embedding-bge-small", "embedding", false),
  makeModel("vad-silero", "vad", false),
];

const config: KnowledgeConfig = {
  dbPath: null,
  modelsDir: null,
  embeddingModel: "embedding-minilm-zh",
  modelMirror: null,
};

function setup(overrides: Partial<Parameters<typeof KnowledgeSettingsSection>[0]> = {}) {
  const handlers = { onSave: vi.fn() };
  render(
    <KnowledgeSettingsSection
      config={config}
      storagePaths={storagePaths}
      models={models}
      {...handlers}
      {...overrides}
    />
  );
  return handlers;
}

describe("KnowledgeSettingsSection", () => {
  it("无自定义路径时展示当前生效路径作为占位", () => {
    setup();
    expect(screen.getByPlaceholderText("C:/data/ai-interview.db")).toBeTruthy();
    expect(screen.getByPlaceholderText("C:/data/models")).toBeTruthy();
  });

  it("嵌入模型下拉仅包含 embedding 类别并标注安装状态", () => {
    setup();
    const select = screen.getByLabelText("嵌入模型") as HTMLSelectElement;
    const labels = Array.from(select.options).map((o) => o.textContent);
    expect(labels).toEqual([
      "embedding-minilm-zh（已安装）",
      "embedding-bge-small（未下载）",
    ]);
  });

  it("修改路径后提示需重启", () => {
    setup();
    fireEvent.change(screen.getByLabelText("数据库路径"), {
      target: { value: "D:/data/ai.db" },
    });
    expect(screen.getByText(/路径变更需重启应用后生效/)).toBeTruthy();
  });

  it("保存回传草稿（含路径与嵌入模型）", () => {
    const h = setup();
    fireEvent.change(screen.getByLabelText("数据库路径"), {
      target: { value: "D:/data/ai.db" },
    });
    fireEvent.change(screen.getByLabelText("嵌入模型"), {
      target: { value: "embedding-bge-small" },
    });
    fireEvent.click(screen.getByText("保存配置"));

    expect(h.onSave).toHaveBeenCalledWith({
      dbPath: "D:/data/ai.db",
      modelsDir: null,
      embeddingModel: "embedding-bge-small",
      modelMirror: null,
    });
  });

  it("可恢复默认路径", () => {
    const h = setup({
      config: { ...config, dbPath: "D:/custom/ai.db" },
    });

    fireEvent.click(screen.getAllByTitle("恢复默认路径")[0]);
    fireEvent.click(screen.getByText("保存配置"));

    expect(h.onSave).toHaveBeenCalledWith(
      expect.objectContaining({ dbPath: null })
    );
  });

  it("清空路径输入即视为使用默认路径", () => {
    const h = setup({ config: { ...config, modelsDir: "D:/custom/models" } });

    fireEvent.change(screen.getByLabelText("模型目录"), { target: { value: "" } });
    fireEvent.click(screen.getByText("保存配置"));

    expect(h.onSave).toHaveBeenCalledWith(
      expect.objectContaining({ modelsDir: null })
    );
  });
});
