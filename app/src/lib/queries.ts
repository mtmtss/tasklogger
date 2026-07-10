import { useEffect } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import {
  getCandidates,
  getCaptures,
  getInboxCount,
  getSyncStatus,
  getTaskLists,
  getTodayDashboard,
} from "./commands";
import type { ActiveSessionView } from "../types";
import { useSessionStore } from "../stores/sessionStore";

export const useDashboard = () =>
  useQuery({ queryKey: ["dashboard"], queryFn: getTodayDashboard });

export const useCandidates = () =>
  useQuery({ queryKey: ["candidates"], queryFn: getCandidates });

export const useSyncStatus = () =>
  useQuery({ queryKey: ["syncStatus"], queryFn: getSyncStatus });

export const useCaptures = () =>
  useQuery({ queryKey: ["captures"], queryFn: getCaptures });

export const useInboxCount = () =>
  useQuery({ queryKey: ["inboxCount"], queryFn: getInboxCount });

export const useTaskLists = () =>
  useQuery({ queryKey: ["taskLists"], queryFn: getTaskLists });

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
      void queryClient.invalidateQueries({ queryKey: ["syncStatus"] });
      void queryClient.invalidateQueries({ queryKey: ["taskLists"] });
    });
    const unlistenSync = listen("sync-status-changed", () => {
      void queryClient.invalidateQueries({ queryKey: ["syncStatus"] });
    });
    const unlistenCaptures = listen("captures-changed", () => {
      void queryClient.invalidateQueries({ queryKey: ["captures"] });
      void queryClient.invalidateQueries({ queryKey: ["inboxCount"] });
    });
    return () => {
      void unlistenSession.then((fn) => fn());
      void unlistenTasks.then((fn) => fn());
      void unlistenSync.then((fn) => fn());
      void unlistenCaptures.then((fn) => fn());
    };
  }, [queryClient, setSession]);
}
