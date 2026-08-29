export type AppStatus = "not_started" | "running" | "paused" | "completed";

export interface TaskItem {
  taskListId: string;
  taskListName: string;
  taskId: string;
  title: string;
  notes: string;
  due: string | null;
  status: string; // Google 側: 'needsAction' | 'completed'
  appStatus: AppStatus;
  todayDurationSeconds: number;
  todayDurationMinutes: number;
  isOverdue: boolean;
}

export interface TaskGroup {
  taskListId: string;
  taskListName: string;
  tasks: TaskItem[];
}

export interface ActiveSessionView {
  taskListId: string;
  taskListName: string;
  taskId: string;
  taskTitle: string;
  startAt: string; // ISO 8601 UTC
  elapsedSeconds: number;
}

export interface TaskListSummary {
  taskListId: string;
  taskListName: string;
  totalSeconds: number;
  totalMinutes: number;
}

export interface AnalyticsSummary {
  totalSeconds: number;
  totalMinutes: number;
  totalSessions: number;
  runningSeconds: number;
  pausedTaskCount: number;
  byTaskList: TaskListSummary[];
}

export interface TodayDashboard {
  dateText: string;
  activeSession: ActiveSessionView | null;
  taskGroups: TaskGroup[];
  analytics: AnalyticsSummary;
}

export interface TaskRef {
  taskListId: string;
  taskId: string;
}

export interface TaskListOption {
  taskListId: string;
  taskListName: string;
}
