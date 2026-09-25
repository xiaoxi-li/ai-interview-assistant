import { describe, expect, it } from "vitest";
import {
  buildGroupTree,
  canMoveGroup,
  collectDescendantIds,
  countNotesByGroup,
  filterNotesByGroup,
  reorderSiblings,
} from "../groupTree";
import type { Group, Note } from "../../types";

const groups: Group[] = [
  { id: "root1", name: "前端", parentId: null, order: 0 },
  { id: "root2", name: "后端", parentId: null, order: 1 },
  { id: "child1", name: "React", parentId: "root1", order: 0 },
  { id: "child2", name: "Vue", parentId: "root1", order: 1 },
  { id: "grand1", name: "Hooks", parentId: "child1", order: 0 },
];

function note(id: string, groupId: string | null): Note {
  return {
    id,
    title: id,
    content: "",
    groupId,
    tags: [],
    isDeleted: false,
    createdAt: 0,
    updatedAt: 0,
  };
}

describe("buildGroupTree", () => {
  it("构建多级树并标注深度", () => {
    const tree = buildGroupTree(groups);
    expect(tree.map((n) => n.id)).toEqual(["root1", "root2"]);
    expect(tree[0].depth).toBe(0);
    expect(tree[0].children.map((n) => n.id)).toEqual(["child1", "child2"]);
    expect(tree[0].children[0].depth).toBe(1);
    expect(tree[0].children[0].children[0].id).toBe("grand1");
    expect(tree[0].children[0].children[0].depth).toBe(2);
  });

  it("孤儿群组（父级缺失）按根节点处理", () => {
    const tree = buildGroupTree([
      { id: "x", name: "孤儿", parentId: "missing", order: 0 },
    ]);
    expect(tree.map((n) => n.id)).toEqual(["x"]);
  });

  it("同级按 order 排序", () => {
    const tree = buildGroupTree([
      { id: "b", name: "B", parentId: null, order: 2 },
      { id: "a", name: "A", parentId: null, order: 1 },
    ]);
    expect(tree.map((n) => n.id)).toEqual(["a", "b"]);
  });
});

describe("collectDescendantIds", () => {
  it("包含自身与全部后代", () => {
    const ids = collectDescendantIds(groups, "root1").sort();
    expect(ids).toEqual(["child1", "child2", "grand1", "root1"]);
  });

  it("叶子节点仅返回自身", () => {
    expect(collectDescendantIds(groups, "grand1")).toEqual(["grand1"]);
  });
});

describe("filterNotesByGroup", () => {
  const notes = [
    note("n-root1", "root1"),
    note("n-grand1", "grand1"),
    note("n-root2", "root2"),
    note("n-none", null),
  ];

  it("undefined 返回全部笔记", () => {
    expect(filterNotesByGroup(notes, groups, undefined)).toHaveLength(4);
  });

  it("null 仅返回未分组笔记", () => {
    expect(filterNotesByGroup(notes, groups, null).map((n) => n.id)).toEqual([
      "n-none",
    ]);
  });

  it("指定群组时包含后代笔记", () => {
    expect(
      filterNotesByGroup(notes, groups, "root1")
        .map((n) => n.id)
        .sort()
    ).toEqual(["n-grand1", "n-root1"]);
  });
});

describe("countNotesByGroup", () => {
  it("仅统计直接归属的笔记", () => {
    const counts = countNotesByGroup([
      note("a", "root1"),
      note("b", "root1"),
      note("c", null),
    ]);
    expect(counts).toEqual({ root1: 2 });
  });
});

describe("canMoveGroup", () => {
  it("允许移动到根级", () => {
    expect(canMoveGroup(groups, "root1", null)).toBe(true);
  });

  it("拒绝移入自身或后代", () => {
    expect(canMoveGroup(groups, "root1", "root1")).toBe(false);
    expect(canMoveGroup(groups, "root1", "grand1")).toBe(false);
  });

  it("允许移入无血缘关系的群组", () => {
    expect(canMoveGroup(groups, "root1", "root2")).toBe(true);
  });
});

describe("reorderSiblings", () => {
  it("同级拖拽生成新的顺序", () => {
    const order = reorderSiblings(groups, "child2", "child1");
    expect(order).toEqual(["child2", "child1"]);
  });

  it("跨层级拖拽不产生重排结果", () => {
    expect(reorderSiblings(groups, "child1", "root2")).toBeNull();
  });
});
