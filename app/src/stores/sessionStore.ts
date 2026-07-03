import { create } from "zustand";
import type { ActiveSessionView } from "../types";
import { getTodayDashboard } from "../lib/commands";

interface SessionState {
  session: ActiveSessionView | null;
  loaded: boolean;
  setSession: (session: ActiveSessionView | null) => void;
  /** 起動直後に現在のセッションを取得する (イベントが来る前の初期値) */
  init: () => Promise<void>;
}

export const useSessionStore = create<SessionState>((set, get) => ({
  session: null,
  loaded: false,
  setSession: (session) => set({ session, loaded: true }),
  init: async () => {
    if (get().loaded) return;
    try {
      const dashboard = await getTodayDashboard();
      set({ session: dashboard.activeSession, loaded: true });
    } catch {
      set({ loaded: true });
    }
  },
}));
