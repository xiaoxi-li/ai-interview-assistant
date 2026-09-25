import type { Group, Note } from "../types";

export interface GroupNode extends Group {
  children: GroupNode[];
  depth: number;
}

/** 将扁平群组列表构建为多级树（同级按 order、名称排序） */
export function buildGroupTree(groups: Group[]): GroupNode[] {
  const byId = new Map<string, GroupNode>();
  groups.forEach((g) => byId.set(g.id, { ...g, children: [], depth: 0 }));

  const roots: GroupNode[] = [];
  byId.forEach((node) => {
    const parent = node.parentId ? byId.get(node.parentId) : undefined;
    if (parent) {
      parent.children.push(node);
    } else {
      roots.push(node);
    }
  });

  const sortRecursively = (nodes: GroupNode[], depth: number) => {
    nodes.sort((a, b) => a.order - b.order || a.name.localeCompare(b.name));
    nodes.forEach((node) => {
      node.depth = depth;
      sortRecursively(node.children, depth + 1);
    });
  };
  sortRecursively(roots, 0);
  return roots;
}

/** 收集群组自身与全部后代 ID */
export function collectDescendantIds(groups: Group[], groupId: string): string[] {
  const childrenOf = new Map<string, string[]>();
  groups.forEach((g) => {
    if (!g.parentId) return;
    const list = childrenOf.get(g.parentId) ?? [];
    list.push(g.id);
    childrenOf.set(g.parentId, list);
  });

  const result: string[] = [];
  const stack = [groupId];
  while (stack.length > 0) {
    const current = stack.pop() as string;
    result.push(current);
    stack.push(...(childrenOf.get(current) ?? []));
  }
  return result;
}

/**
 * 按群组筛选笔记：
 * @param groupId undefined = 全部笔记；null = 仅未分组；字符串 = 该群组及其后代
 */
export function filterNotesByGroup(
  notes: Note[],
  groups: Group[],
  groupId: string | null | undefined
): Note[] {
  if (groupId === undefined) return notes;
  if (groupId === null) return notes.filter((n) => n.groupId === null);
  const ids = new Set(collectDescendantIds(groups, groupId));
  return notes.filter((n) => n.groupId !== null && ids.has(n.groupId));
}

/** 统计各群组直接归属的笔记数量 */
export function countNotesByGroup(notes: Note[]): Record<string, number> {
  const counts: Record<string, number> = {};
  notes.forEach((n) => {
    if (!n.groupId) return;
    counts[n.groupId] = (counts[n.groupId] ?? 0) + 1;
  });
  return counts;
}

/** 校验群组移动是否合法（不能移入自身或其后代） */
export function canMoveGroup(
  groups: Group[],
  groupId: string,
  targetId: string | null
): boolean {
  if (targetId === null) return true;
  if (groupId === targetId) return false;
  return !collectDescendantIds(groups, groupId).includes(targetId);
}

/** 同级重排：把 draggedId 移动到 targetId 所在位置，返回新的同级顺序 */
export function reorderSiblings(
  groups: Group[],
  draggedId: string,
  targetId: string
): string[] | null {
  const dragged = groups.find((g) => g.id === draggedId);
  const target = groups.find((g) => g.id === targetId);
  if (!dragged || !target || dragged.parentId !== target.parentId) return null;

  const siblings = groups
    .filter((g) => g.parentId === target.parentId)
    .sort((a, b) => a.order - b.order || a.name.localeCompare(b.name))
    .map((g) => g.id);

  const from = siblings.indexOf(draggedId);
  const to = siblings.indexOf(targetId);
  if (from === -1 || to === -1) return null;

  siblings.splice(from, 1);
  siblings.splice(to, 0, draggedId);
  return siblings;
}
