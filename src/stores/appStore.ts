import { create } from "zustand";

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
  setCurrentPage: (page: PageKey) => void;
  setOnboarded: (value: boolean) => void;
}

export const useAppStore = create<AppState>((set) => ({
  currentPage: "knowledge",
  onboarded: false,
  setCurrentPage: (page) => set({ currentPage: page }),
  setOnboarded: (value) => set({ onboarded: value }),
}));
