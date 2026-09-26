import {
  BookOpen,
  History,
  Mic,
  Minimize2,
  Minus,
  Settings as SettingsIcon,
  Square,
  X,
} from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { cn, isTauri } from "./lib/utils";
import { isMiniWindow, openMiniWindow } from "./lib/window";
import { useAppStore, type PageKey } from "./stores/appStore";
import KnowledgePage from "./pages/KnowledgePage";
import InterviewPage from "./pages/InterviewPage";
import SessionsPage from "./pages/SessionsPage";
import SettingsPage from "./pages/SettingsPage";
import MiniView from "./components/interview/MiniView";

const NAV_ITEMS: { key: PageKey; label: string; icon: typeof BookOpen }[] = [
  { key: "knowledge", label: "知识库", icon: BookOpen },
  { key: "interview", label: "面试辅助", icon: Mic },
  { key: "sessions", label: "会话历史", icon: History },
  { key: "settings", label: "设置", icon: SettingsIcon },
];

/** 窗口控制（非 Tauri 环境降级为无操作，便于浏览器内独立开发调试） */
type WindowAction = "minimize" | "toggleMaximize" | "close";

function controlWindow(action: WindowAction) {
  if (!isTauri()) return;
  const win = getCurrentWindow();
  const run =
    action === "minimize"
      ? win.minimize()
      : action === "toggleMaximize"
        ? win.toggleMaximize()
        : win.close();
  run.catch((err) => console.error("window control failed:", err));
}

export default function App() {
  const currentPage = useAppStore((s) => s.currentPage);
  const setCurrentPage = useAppStore((s) => s.setCurrentPage);

  // 迷你浮窗：仅渲染精简视图
  if (isMiniWindow()) {
    return <MiniView />;
  }

  return (
    <div className="flex h-full w-full flex-col overflow-hidden bg-background text-text">
      {/* 自定义标题栏 */}
      <header
        data-tauri-drag-region
        className="flex h-11 shrink-0 items-center justify-between border-b border-white/[0.06] bg-background-2/70 px-3 backdrop-blur-xl"
      >
        <div data-tauri-drag-region className="flex items-center gap-2.5">
          <div className="flex h-6 w-6 items-center justify-center rounded-md bg-gradient-to-br from-primary to-primary-3 shadow-md shadow-primary/30">
            <Mic className="h-3.5 w-3.5 text-white" />
          </div>
          <span className="text-[13px] font-medium tracking-wide">
            AI 面试助手
          </span>
          <span className="rounded border border-primary/20 bg-primary/10 px-1.5 py-[1px] text-[10px] text-primary-3">
            v0.1.0
          </span>
        </div>

        <div className="flex items-center gap-0.5">
          <button
            title="迷你模式（仅显示当前问题与 AI 回答）"
            onClick={() => openMiniWindow().catch((err) => console.error(err))}
            className="flex h-7 w-8 cursor-pointer items-center justify-center rounded text-text-subtle transition-colors hover:bg-primary/20 hover:text-primary-3"
          >
            <Minimize2 className="h-3.5 w-3.5" />
          </button>
          <button
            title="最小化"
            onClick={() => controlWindow("minimize")}
            className="flex h-7 w-8 cursor-pointer items-center justify-center rounded text-text-subtle transition-colors hover:bg-white/5 hover:text-text"
          >
            <Minus className="h-3.5 w-3.5" />
          </button>
          <button
            title="最大化"
            onClick={() => controlWindow("toggleMaximize")}
            className="flex h-7 w-8 cursor-pointer items-center justify-center rounded text-text-subtle transition-colors hover:bg-white/5 hover:text-text"
          >
            <Square className="h-3 w-3" />
          </button>
          <button
            title="关闭"
            onClick={() => controlWindow("close")}
            className="flex h-7 w-8 cursor-pointer items-center justify-center rounded text-text-subtle transition-colors hover:bg-danger/80 hover:text-white"
          >
            <X className="h-3.5 w-3.5" />
          </button>
        </div>
      </header>

      <div className="flex min-h-0 flex-1">
        {/* 侧边导航 */}
        <nav className="flex w-[74px] shrink-0 flex-col items-center gap-1 border-r border-white/[0.06] bg-background-2/40 py-3">
          {NAV_ITEMS.map(({ key, label, icon: Icon }) => (
            <button
              key={key}
              onClick={() => setCurrentPage(key)}
              className={cn(
                "group flex w-[60px] cursor-pointer flex-col items-center gap-1 rounded-lg py-2.5 text-[11px] transition-all duration-200",
                currentPage === key
                  ? "bg-primary/15 text-primary-3"
                  : "text-text-subtle hover:bg-white/[0.04] hover:text-text-muted"
              )}
            >
              <Icon className="h-[18px] w-[18px]" />
              <span>{label}</span>
            </button>
          ))}
        </nav>

        {/* 内容区 */}
        <main className="min-w-0 flex-1 overflow-hidden">
          {currentPage === "knowledge" && <KnowledgePage />}
          {currentPage === "interview" && <InterviewPage />}
          {currentPage === "sessions" && <SessionsPage />}
          {currentPage === "settings" && <SettingsPage />}
        </main>
      </div>
    </div>
  );
}
