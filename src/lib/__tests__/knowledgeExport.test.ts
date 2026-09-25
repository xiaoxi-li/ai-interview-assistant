import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../ipc", () => ({ ipc: vi.fn() }));

import { ipc } from "../ipc";
import { useKnowledgeStore } from "../../stores/knowledgeStore";

const mockedIpc = vi.mocked(ipc);

describe("knowledgeStore 导出与备份", () => {
  beforeEach(() => {
    mockedIpc.mockReset();
    useKnowledgeStore.setState({ notice: null, error: null });
  });

  it("导出 Markdown 后展示成功提示", async () => {
    mockedIpc.mockResolvedValueOnce({ files: 3, dest: "D:/out" });

    await useKnowledgeStore.getState().exportMarkdown("D:/out");

    expect(mockedIpc).toHaveBeenCalledWith("export_notes_markdown", {
      dest: "D:/out",
    });
    expect(useKnowledgeStore.getState().notice).toContain("3 篇");
  });

  it("导出 JSON 备份", async () => {
    mockedIpc.mockResolvedValueOnce({ files: 2, dest: "D:/b.json" });

    await useKnowledgeStore.getState().exportJson("D:/b.json");

    expect(mockedIpc).toHaveBeenCalledWith("export_backup_json", {
      dest: "D:/b.json",
    });
    expect(useKnowledgeStore.getState().notice).toContain("2 篇");
  });

  it("数据库文件备份", async () => {
    mockedIpc.mockResolvedValueOnce({ files: 1, dest: "D:/b.sqlite" });

    await useKnowledgeStore.getState().backupDatabase("D:/b.sqlite");

    expect(mockedIpc).toHaveBeenCalledWith("backup_database", {
      dest: "D:/b.sqlite",
    });
    expect(useKnowledgeStore.getState().notice).toContain("D:/b.sqlite");
  });

  it("恢复备份后重新加载数据", async () => {
    mockedIpc.mockResolvedValueOnce({ groups: 1, tags: 2, notes: 4 });
    mockedIpc.mockResolvedValue([]);

    await useKnowledgeStore.getState().restoreBackup("D:/b.json");

    expect(mockedIpc).toHaveBeenCalledWith("restore_backup", {
      source: "D:/b.json",
    });
    expect(useKnowledgeStore.getState().notice).toContain("4 篇");
    // 恢复后触发了数据重载（list_notes 等）
    expect(mockedIpc).toHaveBeenCalledWith("list_notes");
  });

  it("导出失败时记录错误并保持提示为空", async () => {
    mockedIpc.mockRejectedValueOnce(new Error("disk full"));
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});

    await useKnowledgeStore.getState().exportJson("D:/b.json");

    expect(useKnowledgeStore.getState().error).toContain("disk full");
    expect(useKnowledgeStore.getState().notice).toBeNull();
    spy.mockRestore();
  });

  it("clearNotice 清空提示", () => {
    useKnowledgeStore.setState({ notice: "已导出" });

    useKnowledgeStore.getState().clearNotice();

    expect(useKnowledgeStore.getState().notice).toBeNull();
  });
});
