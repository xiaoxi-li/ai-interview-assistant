import CodeMirror from "@uiw/react-codemirror";
import { markdown } from "@codemirror/lang-markdown";
import EditorToolbar from "./EditorToolbar";
import MarkdownView from "./MarkdownView";
import { cn } from "../../lib/utils";
import type { ViewMode } from "../../types";

interface MarkdownEditorProps {
  title: string;
  content: string;
  viewMode: ViewMode;
  saving: boolean;
  savedAt: number | null;
  onTitleChange: (value: string) => void;
  onContentChange: (value: string) => void;
  onViewModeChange: (mode: ViewMode) => void;
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
  onTitleChange,
  onContentChange,
  onViewModeChange,
}: MarkdownEditorProps) {
  const showSource = viewMode !== "preview";
  const showPreview = viewMode !== "source";

  return (
    <div className="flex h-full flex-col">
      <EditorToolbar
        title={title}
        viewMode={viewMode}
        saving={saving}
        savedAt={savedAt}
        onTitleChange={onTitleChange}
        onViewModeChange={onViewModeChange}
      />

      <div className="flex min-h-0 flex-1">
        {showSource && (
          <div
            className={cn(
              "min-w-0 overflow-hidden",
              viewMode === "split" ? "w-1/2 border-r border-white/5" : "flex-1"
            )}
          >
            <CodeMirror
              value={content}
              height="100%"
              theme="dark"
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
            <MarkdownView content={content} />
          </div>
        )}
      </div>
    </div>
  );
}
