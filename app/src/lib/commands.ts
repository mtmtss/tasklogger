import { invoke } from "@tauri-apps/api/core";
import type { TaskGroup, TaskRef, TodayDashboard } from "../types";

export const getTodayDashboard = () =>
  invoke<TodayDashboard>("get_today_dashboard");

export const getCandidates = () => invoke<TaskGroup[]>("get_candidates");

export const startTask = (task: TaskRef) => invoke<void>("start_task", { task });

export const stopTask = (action: "paused" | "completed", memo?: string) =>
  invoke<void>("stop_task", { action, memo: memo ?? null });

export const completeTaskDirect = (task: TaskRef, memo?: string) =>
  invoke<void>("complete_task_direct", { task, memo: memo ?? null });

export const doItNow = (task: TaskRef) => invoke<void>("do_it_now", { task });

export const getSettings = () =>
  invoke<Record<string, string>>("get_settings");

export const setSetting = (key: string, value: string) =>
  invoke<void>("set_setting", { key, value });

export const seedSampleData = () => invoke<void>("seed_sample_data");

export const toggleFloatWindow = () => invoke<boolean>("toggle_float_window");

export interface SyncStatus {
  connected: boolean;
  lastPullAt: string | null;
  queueCount: number;
}

export const connectGoogle = (clientId: string, clientSecret: string) =>
  invoke<void>("connect_google", { clientId, clientSecret });

export const disconnectGoogle = () => invoke<void>("disconnect_google");

export const syncNow = () => invoke<void>("sync_now");

export const getSyncStatus = () => invoke<SyncStatus>("get_sync_status");

export interface InterruptedTask {
  taskListId: string;
  taskId: string;
  taskTitle: string;
}

export const getInterruptedTask = () =>
  invoke<InterruptedTask | null>("get_interrupted_task");

export const resumeInterrupted = () => invoke<void>("resume_interrupted");

export const dismissInterrupted = () => invoke<void>("dismiss_interrupted");
