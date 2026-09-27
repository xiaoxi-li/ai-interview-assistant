import { create } from "zustand";
import { ipc } from "../lib/ipc";
import { isTauri } from "../lib/utils";
import { loadLocale, saveLocale, type Locale } from "../lib/i18n";

export type PageKey =
  | "knowledge"
  | "interview"
  | "sessions"
  | "settings"
  | "onboarding";

interface AppState {
  /** 当前页面 */
  currentPage: PageKey;
  /** 是否已完成首次启动向导 */
  onboarded: boolean;
  /** 向导状态是否已从后端读取（避免首帧闪烁） */
  onboardingLoaded: boolean;
  /** 界面语言 */
  locale: Locale;
  /** 是否处于迷你模式（IA-13，会话内有效，不持久化） */
  miniMode: boolean;
  setCurrentPage: (page: PageKey) => void;
  setOnboarded: (value: boolean) => void;
  /** 切换界面语言（本地持久化） */
  setLocale: (locale: Locale) => void;
  /** 读取向导完成状态 */
  loadOnboarding: () => Promise<void>;
  /** 标记向导完成并进入主界面 */
  completeOnboarding: () => Promise<void>;
  /** 进入迷你模式：界面切为迷你页，并请求后端把主窗口缩为小尺寸置顶浮窗 */
  enterMiniMode: () => void;
  /** 退出迷你模式：还原完整界面，并请求后端还原窗口尺寸/位置/置顶 */
  exitMiniMode: () => void;
  /** 切换迷你模式（标题栏按钮、应用内/全局热键、托盘共用入口） */
  toggleMiniMode: () => void;
}

export const useAppStore = create<AppState>((set, get) => ({
  currentPage: "knowledge",
  onboarded: false,
  onboardingLoaded: false,
  locale: loadLocale(),
  miniMode: false,
  setCurrentPage: (page) => set({ currentPage: page }),
  setOnboarded: (value) => set({ onboarded: value }),
  setLocale: (locale) => {
    saveLocale(locale);
    set({ locale });
  },

  loadOnboarding: () =>
    ipc<boolean>("get_onboarding_state")
      .then((onboarded) => set({ onboarded, onboardingLoaded: true }))
      .catch((err) => {
        console.error("读取向导状态失败:", err);
        // 读取失败时按「已完成」处理，避免把用户困在向导里
        set({ onboarded: true, onboardingLoaded: true });
      }),

  completeOnboarding: () =>
    ipc<boolean>("complete_onboarding", { completed: true })
      .then((onboarded) => set({ onboarded, onboardingLoaded: true, currentPage: "knowledge" }))
      .catch((err) => {
        console.error("保存向导状态失败:", err);
        set({ onboarded: true, onboardingLoaded: true, currentPage: "knowledge" });
      }),

  enterMiniMode: () => {
    if (get().miniMode) return;
    set({ miniMode: true });
    if (!isTauri()) return;
    ipc<void>("enter_mini_mode").catch((err) => {
      console.error("进入迷你模式失败:", err);
      // 后端窗口调整失败时回退视图，避免出现「小窗未生效却只见迷你页」的错位
      set({ miniMode: false });
    });
  },

  exitMiniMode: () => {
    if (!get().miniMode) return;
    // 退出后统一回到面试辅助页，符合面试场景的使用预期
    set({ miniMode: false, currentPage: "interview" });
    if (!isTauri()) return;
    ipc<void>("exit_mini_mode").catch((err) => console.error("退出迷你模式失败:", err));
  },

  toggleMiniMode: () => (get().miniMode ? get().exitMiniMode() : get().enterMiniMode()),
}));
