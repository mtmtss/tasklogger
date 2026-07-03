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
