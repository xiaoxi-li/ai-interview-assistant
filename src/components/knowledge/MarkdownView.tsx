import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";

interface MarkdownViewProps {
  content: string;
  /** 是否限制正文最大宽度（阅读模式用） */
  readable?: boolean;
}

/** Markdown 预览：支持 GFM 表格、任务列表、代码高亮、图片 */
export default function MarkdownView({
  content,
  readable = true,
}: MarkdownViewProps) {
  return (
    <div className={`markdown-body ${readable ? "mx-auto max-w-3xl" : ""}`}>
      <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
        {content}
      </ReactMarkdown>
    </div>
  );
}
