import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../ipc", () => ({ ipc: vi.fn() }));

import { ipc } from "../ipc";
import { useKnowledgeStore } from "../../stores/knowledgeStore";

const mockedIpc = vi.mocked(ipc);

function resetStore() {
  useKnowledgeStore.setState({
    hits: [],
    searchMode: "keyword",
    keyword: "",
    error: null,
  });
}

describe("knowledgeStore 检索分发", () => {
  beforeEach(() => {
    mockedIpc.mockReset();
    resetStore();
  });

  it("关键词模式下调用全文检索命令并写入命中", async () => {
    const hits = [
      { noteId: "n1", title: "架构", snippet: "……架构……", score: 1 },
    ];
    mockedIpc.mockResolvedValueOnce(hits);

    await useKnowledgeStore.getState().search("架构");

    expect(mockedIpc).toHaveBeenCalledWith("search_notes", {
      query: "架构",
      limit: 50,
    });
    expect(useKnowledgeStore.getState().hits).toEqual(hits);
  });

  it("语义模式下调用向量检索命令", async () => {
    mockedIpc.mockResolvedValueOnce([]);
    useKnowledgeStore.setState({ searchMode: "semantic" });

    await useKnowledgeStore.getState().search("项目架构");

    expect(mockedIpc).toHaveBeenCalledWith("search_similar_notes", {
      query: "项目架构",
      topK: 20,
    });
  });

  it("混合模式下调用 RRF 融合检索命令", async () => {
    mockedIpc.mockResolvedValueOnce([]);
    useKnowledgeStore.setState({ searchMode: "hybrid" });

    await useKnowledgeStore.getState().search("项目架构");

    expect(mockedIpc).toHaveBeenCalledWith("search_hybrid_notes", {
      query: "项目架构",
      topK: 20,
    });
  });

  it("空关键词清空命中且不发请求", async () => {
    useKnowledgeStore.setState({
      hits: [{ noteId: "n1", title: "t", snippet: "s", score: 1 }],
    });

    await useKnowledgeStore.getState().search("   ");

    expect(mockedIpc).not.toHaveBeenCalled();
    expect(useKnowledgeStore.getState().hits).toEqual([]);
  });

  it("检索失败时记录错误并清空命中", async () => {
    mockedIpc.mockRejectedValueOnce(new Error("boom"));
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});

    await useKnowledgeStore.getState().search("架构");

    expect(useKnowledgeStore.getState().hits).toEqual([]);
    expect(useKnowledgeStore.getState().error).toContain("boom");
    spy.mockRestore();
  });

  it("切换检索模式时用当前关键词重查", () => {
    mockedIpc.mockResolvedValueOnce([]);
    useKnowledgeStore.setState({ keyword: "架构" });

    useKnowledgeStore.getState().setSearchMode("semantic");

    expect(useKnowledgeStore.getState().searchMode).toBe("semantic");
    expect(mockedIpc).toHaveBeenCalledWith("search_similar_notes", {
      query: "架构",
      topK: 20,
    });
  });
});
