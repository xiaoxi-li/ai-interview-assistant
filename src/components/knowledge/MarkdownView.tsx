import { memo, useMemo } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";

interface MarkdownViewProps {
  content: string;
  /** 是否限制正文最大宽度（阅读模式用） */
  readable?: boolean;
}

/**
 * Markdown 预览：支持 GFM 表格、任务列表、代码高亮、图片。
 *
 * 解析 + 语法高亮是同步重活，故按 `content` 记忆化，并用 `memo` 对外隔离：父组件因
 * 保存状态、标签、主题等无关变化重渲染时，直接复用同一元素让 React 跳过整棵子树（NFR-17）。
 */
function MarkdownView({ content, readable = true }: MarkdownViewProps) {
  const body = useMemo(
    () => (
      <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
        {content}
      </ReactMarkdown>
    ),
    [content]
  );

  return (
    <div className={`markdown-body ${readable ? "mx-auto max-w-3xl" : ""}`}>{body}</div>
  );
}

export default memo(MarkdownView);
