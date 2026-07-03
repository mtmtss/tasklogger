import { useEffect } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { getCandidates, getTodayDashboard } from "./commands";
import type { ActiveSessionView } from "../types";
import { useSessionStore } from "../stores/sessionStore";

export const useDashboard = () =>
  useQuery({ queryKey: ["dashboard"], queryFn: getTodayDashboard });

export const useCandidates = () =>
  useQuery({ queryKey: ["candidates"], queryFn: getCandidates });

/**
 * Rust 側からのイベントを購読して Query を invalidate し、
 * session-changed は Zustand store にも反映する (spec §6.5, §8.3)。
 * main / float どちらのウィンドウでもルートで 1 回呼ぶ。
 */
export function useTauriEvents() {
  const queryClient = useQueryClient();
  const setSession = useSessionStore((s) => s.setSession);

  useEffect(() => {
    const unlistenSession = listen<ActiveSessionView | null>(
      "session-changed",
      (event) => {
        setSession(event.payload);
        void queryClient.invalidateQueries({ queryKey: ["dashboard"] });
      },
    );
    const unlistenTasks = listen("tasks-changed", () => {
      void queryClient.invalidateQueries({ queryKey: ["dashboard"] });
      void queryClient.invalidateQueries({ queryKey: ["candidates"] });
    });
    return () => {
      void unlistenSession.then((fn) => fn());
      void unlistenTasks.then((fn) => fn());
    };
  }, [queryClient, setSession]);
}
