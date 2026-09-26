import { create } from "zustand";
import { ipc } from "../lib/ipc";

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
  setCurrentPage: (page: PageKey) => void;
  setOnboarded: (value: boolean) => void;
  /** 读取向导完成状态 */
  loadOnboarding: () => Promise<void>;
  /** 标记向导完成并进入主界面 */
  completeOnboarding: () => Promise<void>;
}

export const useAppStore = create<AppState>((set) => ({
  currentPage: "knowledge",
  onboarded: false,
  onboardingLoaded: false,
  setCurrentPage: (page) => set({ currentPage: page }),
  setOnboarded: (value) => set({ onboarded: value }),

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
}));
