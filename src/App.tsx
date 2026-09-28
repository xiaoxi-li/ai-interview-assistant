import { useEffect } from "react";
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
import logoLockupLight from "./assets/logo-lockup-light.png";
import logoLockupDark from "./assets/logo-lockup-dark.png";
import { useT } from "./hooks/useT";
import { useThemeSync } from "./hooks/useThemeSync";
import { useAppStore, type PageKey } from "./stores/appStore";
import KnowledgePage from "./pages/KnowledgePage";
import InterviewPage from "./pages/InterviewPage";
import SessionsPage from "./pages/SessionsPage";
import SettingsPage from "./pages/SettingsPage";
import OnboardingPage from "./pages/OnboardingPage";
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
  const onboarded = useAppStore((s) => s.onboarded);
  const onboardingLoaded = useAppStore((s) => s.onboardingLoaded);
  const loadOnboarding = useAppStore((s) => s.loadOnboarding);
  const miniMode = useAppStore((s) => s.miniMode);
  const enterMiniMode = useAppStore((s) => s.enterMiniMode);
  const t = useT();

  // 主题同步（ST-13）：必须在迷你模式 / 向导早退之前调用，Hooks 不可条件调用
  useThemeSync();

  useEffect(() => {
    loadOnboarding();
  }, [loadOnboarding]);

  // 迷你模式（IA-13）：整个界面就地切换为「仅展示当前问题 + AI 回答」的迷你页
  if (miniMode) {
    return <MiniView />;
  }

  // 首次启动：向导状态读取完成后若未完成则进入向导
  if (!onboardingLoaded) {
    return (
      <div className="flex h-full w-full items-center justify-center bg-background text-[12px] text-text-subtle">
        正在启动…
      </div>
    );
  }

  if (!onboarded) {
    return <OnboardingPage />;
  }

  return (
    <div className="flex h-full w-full flex-col overflow-hidden bg-background text-text">
      {/* 自定义标题栏 */}
      <header
        data-tauri-drag-region
        className="flex h-11 shrink-0 items-center justify-between border-b border-ghost/[0.06] bg-background-2/70 px-3 backdrop-blur-xl"
      >
        <div data-tauri-drag-region className="flex items-center gap-2.5">
          <img
            src={logoLockupLight}
            alt=""
            draggable={false}
            className="h-7 w-auto select-none dark:hidden"
          />
          <img
            src={logoLockupDark}
            alt=""
            draggable={false}
            className="hidden h-7 w-auto select-none dark:block"
          />
          <span className="rounded border border-primary/20 bg-primary/10 px-1.5 py-[1px] text-[10px] text-primary-3">
            v0.1.0
          </span>
        </div>

        <div className="flex items-center gap-0.5">
          <button
            title={t("迷你模式（仅显示当前问题与 AI 回答）")}
            onClick={enterMiniMode}
            className="focus-ring flex h-7 w-8 cursor-pointer items-center justify-center rounded text-text-subtle transition-colors hover:bg-primary/25 hover:text-primary-3"
          >
            <Minimize2 className="h-3.5 w-3.5" />
          </button>
          <button
            title={t("最小化")}
            onClick={() => controlWindow("minimize")}
            className="focus-ring flex h-7 w-8 cursor-pointer items-center justify-center rounded text-text-subtle transition-colors hover:bg-ghost/[0.06] hover:text-text"
          >
            <Minus className="h-3.5 w-3.5" />
          </button>
          <button
            title={t("最大化")}
            onClick={() => controlWindow("toggleMaximize")}
            className="focus-ring flex h-7 w-8 cursor-pointer items-center justify-center rounded text-text-subtle transition-colors hover:bg-ghost/[0.06] hover:text-text"
          >
            <Square className="h-3 w-3" />
          </button>
          <button
            title={t("关闭")}
            onClick={() => controlWindow("close")}
            className="focus-ring flex h-7 w-8 cursor-pointer items-center justify-center rounded text-text-subtle transition-colors hover:bg-danger/80 hover:text-white"
          >
            <X className="h-3.5 w-3.5" />
          </button>
        </div>
      </header>

      <div className="flex min-h-0 flex-1">
        {/* 侧边导航 */}
        <nav className="flex w-[74px] shrink-0 flex-col items-center gap-1 border-r border-ghost/[0.06] bg-background-2/40 py-3">
          {NAV_ITEMS.map(({ key, label, icon: Icon }) => (
            <button
              key={key}
              onClick={() => setCurrentPage(key)}
              className={cn(
                "focus-ring group flex w-[60px] cursor-pointer flex-col items-center gap-1 rounded-lg py-2.5 text-[11px] transition-all duration-200",
                currentPage === key
                  ? "bg-primary/15 text-primary-3"
                  : "text-text-subtle hover:bg-ghost/[0.06] hover:text-text-muted"
              )}
            >
              <Icon className="h-[18px] w-[18px]" />
              <span>{t(label)}</span>
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
