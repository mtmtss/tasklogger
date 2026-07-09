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

export const setAutostart = (enabled: boolean) =>
  invoke<void>("set_autostart", { enabled });

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

export const scheduleForToday = (task: TaskRef) =>
  invoke<void>("schedule_for_today", { task });

// ---- AI 拡張「今日の作戦」 ----

export interface PlanTaskItem {
  taskListId: string | null;
  taskId: string | null;
  title: string;
  firstStep: string;
  estimatedMinutes: number;
  reason: string;
}

export interface PlanNotTodayItem {
  taskListId: string | null;
  taskId: string | null;
  title: string;
  reason: string;
}

export interface DailyPlan {
  must_do: PlanTaskItem[];
  if_possible: PlanTaskItem[];
  five_minute: PlanTaskItem[];
  not_today: PlanNotTodayItem[];
  advice: string;
}

export interface StoredPlan {
  planDate: string;
  generatedAt: string;
  inputNote: string;
  model: string;
  plan: DailyPlan;
}

export interface AiStatus {
  configured: boolean;
  model: string;
}

export const getAiStatus = () => invoke<AiStatus>("get_ai_status");

export const setAiApiKey = (key: string) =>
  invoke<void>("set_ai_api_key", { key });

export const clearAiApiKey = () => invoke<void>("clear_ai_api_key");

export const generateDailyPlan = (note: string) =>
  invoke<StoredPlan>("generate_daily_plan", { note });

export const getDailyPlan = () => invoke<StoredPlan | null>("get_daily_plan");

export interface ReviewIncompleteItem {
  title: string;
  reason: string;
}

export interface ReviewTomorrowItem {
  taskListId: string | null;
  taskId: string | null;
  title: string;
  reason: string;
}

export interface DailyReview {
  done: string[];
  incomplete: ReviewIncompleteItem[];
  tomorrow: ReviewTomorrowItem[];
  research_progress: string;
  summary: string;
}

export interface StoredReview {
  reviewDate: string;
  generatedAt: string;
  model: string;
  review: DailyReview;
}

export const generateDailyReview = () =>
  invoke<StoredReview>("generate_daily_review");

export const getDailyReview = () =>
  invoke<StoredReview | null>("get_daily_review");

// ---- クイックキャプチャ + Inbox (AI 拡張仕様 §13) ----

export type CaptureKind =
  | "TASK"
  | "RESEARCH_IDEA"
  | "LIFE_ADMIN"
  | "SOMEDAY"
  | "UNCLEAR";

export interface CaptureItem {
  kind: CaptureKind;
  title: string;
  listName?: string | null;
  due?: string | null;
  firstStep?: string | null;
  note?: string | null;
  registeredTaskId?: string | null;
}

export interface Capture {
  id: string;
  text: string;
  status: "pending" | "classified" | "registered" | "dismissed";
  aiResult: { items: CaptureItem[] } | null;
  createdAt: string;
}

export interface TaskListOption {
  id: string;
  title: string;
}

export interface RegisterItemInput {
  title: string;
  listId?: string | null;
  listName?: string | null;
  due?: string | null;
  firstStep?: string | null;
  note?: string | null;
  itemIndex?: number | null;
}

export const addCapture = (text: string) =>
  invoke<string>("add_capture", { text });

export const getCaptures = () => invoke<Capture[]>("get_captures");

export const getInboxCount = () => invoke<number>("get_inbox_count");

export const classifyCapture = (captureId: string) =>
  invoke<void>("classify_capture", { captureId });

export const registerCaptureItem = (
  captureId: string,
  item: RegisterItemInput,
) => invoke<Capture>("register_capture_item", { captureId, item });

export const dismissCapture = (captureId: string) =>
  invoke<void>("dismiss_capture", { captureId });

export const quickAddTask = (
  listId: string,
  title: string,
  due?: string | null,
) => invoke<void>("quick_add_task", { listId, title, due: due ?? null });

export const getTaskLists = () =>
  invoke<TaskListOption[]>("get_task_lists");

export const openCaptureWindow = () => invoke<void>("open_capture_window");

export const setCaptureHotkey = (hotkey: string) =>
  invoke<void>("set_capture_hotkey", { hotkey });

export interface DateSummary {
  date: string;
  totalSeconds: number;
  totalMinutes: number;
  sessionCount: number;
}

export interface ListSummary {
  taskListId: string;
  taskListName: string;
  totalSeconds: number;
  totalMinutes: number;
  sessionCount: number;
  completedCount: number;
}

export interface TaskSummary extends ListSummary {
  taskId: string;
  taskTitle: string;
  lastWorkedDate: string;
}

export interface ArchiveAnalytics {
  startDate: string;
  endDate: string;
  totalSeconds: number;
  totalMinutes: number;
  totalSessions: number;
  completedSessions: number;
  activeDays: number;
  averageMinutesPerDay: number;
  byDate: DateSummary[];
  byTaskList: ListSummary[];
  byTask: TaskSummary[];
}

export const getArchiveAnalytics = (startDate: string, endDate: string) =>
  invoke<ArchiveAnalytics>("get_archive_analytics", { startDate, endDate });

export interface ImportResult {
  imported: number;
  skipped: number;
}

/** 保存ダイアログでキャンセルすると null。成功時は保存先パス。 */
export const exportCsv = (startDate: string, endDate: string) =>
  invoke<string | null>("export_csv", { startDate, endDate });

/** ファイル選択でキャンセルすると null。 */
export const importGasCsv = () =>
  invoke<ImportResult | null>("import_gas_csv");
