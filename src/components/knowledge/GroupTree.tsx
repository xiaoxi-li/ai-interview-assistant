import { useState } from "react";
import {
  ChevronDown,
  ChevronRight,
  Folder,
  FolderPlus,
  Layers,
  Pencil,
  Trash2,
} from "lucide-react";
import { cn } from "../../lib/utils";
import { GROUP_DRAG_TYPE, NOTE_DRAG_TYPE } from "../../lib/dnd";
import { buildGroupTree, reorderSiblings, type GroupNode } from "../../lib/groupTree";
import type { Group } from "../../types";

const ICON_BTN =
  "focus-ring flex h-5 w-5 shrink-0 cursor-pointer items-center justify-center rounded text-text-subtle transition-colors";

interface GroupTreeProps {
  groups: Group[];
  /** undefined = 全部笔记；null = 未分组 */
  selectedGroupId: string | null | undefined;
  noteCounts: Record<string, number>;
  totalCount: number;
  ungroupedCount: number;
  onSelect: (groupId: string | null | undefined) => void;
  onCreate: (name: string, parentId: string | null) => void;
  onRename: (id: string, name: string) => void;
  onDelete: (id: string) => void;
  onMove: (id: string, parentId: string | null) => void;
  onReorder: (orderedIds: string[]) => void;
  onDropNote: (noteId: string, groupId: string | null) => void;
}

export default function GroupTree({
  groups,
  selectedGroupId,
  noteCounts,
  totalCount,
  ungroupedCount,
  onSelect,
  onCreate,
  onRename,
  onDelete,
  onMove,
  onReorder,
  onDropNote,
}: GroupTreeProps) {
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [editing, setEditing] = useState<{ id: string | null; parentId: string | null } | null>(
    null
  );
  const [draft, setDraft] = useState("");

  const tree = buildGroupTree(groups);

  const toggle = (id: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const startCreate = (parentId: string | null) => {
    setEditing({ id: null, parentId });
    setDraft("");
    if (parentId) setExpanded((prev) => new Set(prev).add(parentId));
  };

  const commitEdit = () => {
    if (!editing) return;
    const target = editing;
    const name = draft.trim();
    setEditing(null);
    setDraft("");
    if (!name) return;
    if (target.id) onRename(target.id, name);
    else onCreate(name, target.parentId);
  };

  const handleEditKey = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") commitEdit();
    if (e.key === "Escape") {
      setEditing(null);
      setDraft("");
    }
  };

  const handleDrop = (e: React.DragEvent, targetId: string | null) => {
    e.preventDefault();
    e.stopPropagation();

    const noteId = e.dataTransfer.getData(NOTE_DRAG_TYPE);
    if (noteId) {
      onDropNote(noteId, targetId);
      return;
    }

    const groupId = e.dataTransfer.getData(GROUP_DRAG_TYPE);
    if (!groupId || groupId === targetId) return;
    if (targetId === null) {
      onMove(groupId, null);
      return;
    }
    // 同级拖拽 → 重排；跨级拖拽 → 变更父级
    const order = reorderSiblings(groups, groupId, targetId);
    if (order) onReorder(order);
    else onMove(groupId, targetId);
  };

  const renderNode = (node: GroupNode) => {
    const hasChildren = node.children.length > 0;
    const isOpen = expanded.has(node.id);
    const selected = selectedGroupId === node.id;

    return (
      <div key={node.id}>
        <div
          role="button"
          tabIndex={0}
          draggable
          onDragStart={(e) => {
            e.dataTransfer.setData(GROUP_DRAG_TYPE, node.id);
            e.stopPropagation();
          }}
          onDragOver={(e) => e.preventDefault()}
          onDrop={(e) => handleDrop(e, node.id)}
          onClick={() => onSelect(node.id)}
          onKeyDown={(e) => e.key === "Enter" && onSelect(node.id)}
          style={{ paddingLeft: 6 + node.depth * 14 }}
          className={cn(
            "focus-ring group flex cursor-pointer items-center gap-1 rounded-md py-1 pr-1.5 text-[12px] transition-colors",
            selected
              ? "bg-primary/15 text-primary-3"
              : "text-text-muted hover:bg-white/[0.06] hover:text-text"
          )}
        >
          {hasChildren ? (
            <button
              title={isOpen ? "折叠" : "展开"}
              onClick={(e) => {
                e.stopPropagation();
                toggle(node.id);
              }}
              className={cn(ICON_BTN, "hover:text-text")}
            >
              {isOpen ? (
                <ChevronDown className="h-3 w-3" />
              ) : (
                <ChevronRight className="h-3 w-3" />
              )}
            </button>
          ) : (
            <span className="h-5 w-5 shrink-0" />
          )}

          <Folder className="h-3.5 w-3.5 shrink-0" />

          {editing?.id === node.id ? (
            <input
              autoFocus
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={handleEditKey}
              onBlur={commitEdit}
              className="min-w-0 flex-1 rounded border border-primary/40 bg-background-3 px-1 py-0.5 text-[12px] text-text outline-none"
            />
          ) : (
            <span className="min-w-0 flex-1 truncate">{node.name}</span>
          )}

          <span className="text-[10px] text-text-subtle">{noteCounts[node.id] ?? 0}</span>

          <span className="flex items-center gap-0.5 opacity-0 transition-opacity group-hover:opacity-100">
            <button
              title="新建子群组"
              onClick={(e) => {
                e.stopPropagation();
                startCreate(node.id);
              }}
              className={cn(ICON_BTN, "hover:bg-primary/20 hover:text-primary-3")}
            >
              <FolderPlus className="h-3 w-3" />
            </button>
            <button
              title="重命名"
              onClick={(e) => {
                e.stopPropagation();
                setEditing({ id: node.id, parentId: node.parentId });
                setDraft(node.name);
              }}
              className={cn(ICON_BTN, "hover:bg-white/[0.06] hover:text-text")}
            >
              <Pencil className="h-3 w-3" />
            </button>
            <button
              title="删除群组"
              onClick={(e) => {
                e.stopPropagation();
                onDelete(node.id);
              }}
              className={cn(ICON_BTN, "hover:bg-danger/20 hover:text-danger")}
            >
              <Trash2 className="h-3 w-3" />
            </button>
          </span>
        </div>

        {isOpen && node.children.map(renderNode)}
      </div>
    );
  };

  const pseudoRow = (
    key: string,
    icon: typeof Layers,
    label: string,
    count: number,
    value: string | null | undefined
  ) => {
    const Icon = icon;
    const selected = selectedGroupId === value;
    return (
      <div
        key={key}
        role="button"
        tabIndex={0}
        onClick={() => onSelect(value)}
        onKeyDown={(e) => e.key === "Enter" && onSelect(value)}
        onDragOver={(e) => e.preventDefault()}
        onDrop={(e) => handleDrop(e, value === undefined ? null : value)}
        className={cn(
          "focus-ring flex cursor-pointer items-center gap-1.5 rounded-md py-1 pl-3 pr-2 text-[12px] transition-colors",
          selected
            ? "bg-primary/15 text-primary-3"
            : "text-text-muted hover:bg-white/[0.06] hover:text-text"
        )}
      >
        <Icon className="h-3.5 w-3.5 shrink-0" />
        <span className="min-w-0 flex-1 truncate">{label}</span>
        <span className="text-[10px] text-text-subtle">{count}</span>
      </div>
    );
  };

  return (
    <div className="flex flex-col gap-0.5 px-1">
      <div className="mb-1 flex items-center gap-1 px-1">
        <span className="text-[10px] uppercase tracking-wide text-text-subtle">群组</span>
        <button
          title="新建根群组"
          onClick={() => startCreate(null)}
          className={cn(ICON_BTN, "ml-auto hover:bg-primary/20 hover:text-primary-3")}
        >
          <FolderPlus className="h-3 w-3" />
        </button>
      </div>

      {pseudoRow("all", Layers, "全部笔记", totalCount, undefined)}
      {pseudoRow("ungrouped", Folder, "未分组", ungroupedCount, null)}

      {editing && editing.id === null && (
        <input
          autoFocus
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={handleEditKey}
          onBlur={commitEdit}
          placeholder="群组名称"
          className="ml-3 rounded border border-primary/40 bg-background-3 px-1.5 py-1 text-[12px] text-text outline-none"
        />
      )}

      {tree.map(renderNode)}

      {/* 根级放置区 */}
      <div
        onDragOver={(e) => e.preventDefault()}
        onDrop={(e) => handleDrop(e, null)}
        className="mt-1 rounded-lg border border-dashed border-white/10 py-1.5 text-center text-[10px] text-text-subtle"
      >
        拖到此处移到根级
      </div>
    </div>
  );
}
