import { useDeferredValue } from "react";
import CodeMirror from "@uiw/react-codemirror";
import { markdown } from "@codemirror/lang-markdown";
import EditorToolbar from "./EditorToolbar";
import MarkdownView from "./MarkdownView";
import TagEditor from "./TagEditor";
import { cn } from "../../lib/utils";
import { useResolvedTheme } from "../../hooks/useThemeSync";
import type { ViewMode } from "../../types";

interface MarkdownEditorProps {
  title: string;
  content: string;
  viewMode: ViewMode;
  saving: boolean;
  savedAt: number | null;
  /** 笔记标签 */
  tags?: string[];
  /** 全部已有标签（作为输入建议） */
  tagSuggestions?: string[];
  onTitleChange: (value: string) => void;
  onContentChange: (value: string) => void;
  onViewModeChange: (mode: ViewMode) => void;
  onAddTag?: (tag: string) => void;
  onRemoveTag?: (tag: string) => void;
  /** 版本历史开关 */
  historyOpen?: boolean;
  onToggleHistory?: () => void;
}

/**
 * 源码 / 预览双视图编辑器。
 * 说明：源码视图使用 CodeMirror（Markdown 语法高亮），
 * 预览视图支持 GFM 表格、任务列表、代码块与图片。
 */
export default function MarkdownEditor({
  title,
  content,
  viewMode,
  saving,
  savedAt,
  tags = [],
  tagSuggestions = [],
  onTitleChange,
  onContentChange,
  onViewModeChange,
  onAddTag,
  onRemoveTag,
  historyOpen,
  onToggleHistory,
}: MarkdownEditorProps) {
  const showSource = viewMode !== "preview";
  const showPreview = viewMode !== "source";
  // 源码编辑器主题随全局主题切换（ST-13）
  const theme = useResolvedTheme();
  // 预览是同步重活（Markdown 解析 + 语法高亮）：用 deferred 值延后到输入/自动保存的
  // 空闲时段渲染，避免每次按键都被全量重解析打断（NFR-17）
  const deferredContent = useDeferredValue(content);

  return (
    <div className="flex h-full flex-col">
      <EditorToolbar
        title={title}
        viewMode={viewMode}
        saving={saving}
        savedAt={savedAt}
        onTitleChange={onTitleChange}
        onViewModeChange={onViewModeChange}
        historyOpen={historyOpen}
        onToggleHistory={onToggleHistory}
      />

      {onAddTag && onRemoveTag && (
        <TagEditor
          tags={tags}
          suggestions={tagSuggestions}
          onAdd={onAddTag}
          onRemove={onRemoveTag}
        />
      )}

      <div className="flex min-h-0 flex-1">
        {showSource && (
          <div
            className={cn(
              "min-w-0 overflow-hidden",
              viewMode === "split" ? "w-1/2 border-r border-ghost/[0.06]" : "flex-1"
            )}
          >
            <CodeMirror
              value={content}
              height="100%"
              theme={theme}
              extensions={[markdown()]}
              onChange={onContentChange}
              basicSetup={{
                lineNumbers: true,
                foldGutter: false,
                highlightActiveLine: true,
                autocompletion: false,
              }}
            />
          </div>
        )}

        {showPreview && (
          <div
            className={cn(
              "selectable min-w-0 overflow-y-auto px-6 py-4",
              viewMode === "split" ? "w-1/2" : "flex-1"
            )}
          >
            <MarkdownView content={deferredContent} />
          </div>
        )}
      </div>
    </div>
  );
}
