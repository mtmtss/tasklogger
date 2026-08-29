use serde::{Deserialize, Serialize};

/// タスクのアプリ内状態。work_logs + active_session から導出される (spec §4.1)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AppStatus {
    NotStarted,
    Running,
    Paused,
    Completed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskItem {
    pub task_list_id: String,
    pub task_list_name: String,
    pub task_id: String,
    pub title: String,
    pub notes: String,
    pub due: Option<String>,
    /// Google 側の status ('needsAction' | 'completed')
    pub status: String,
    pub app_status: AppStatus,
    pub today_duration_seconds: i64,
    pub today_duration_minutes: i64,
    /// due が今日より前かどうか (期限切れバッジ表示用)。
    pub is_overdue: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskGroup {
    pub task_list_id: String,
    pub task_list_name: String,
    pub tasks: Vec<TaskItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveSessionView {
    pub task_list_id: String,
    pub task_list_name: String,
    pub task_id: String,
    pub task_title: String,
    /// ISO 8601 UTC
    pub start_at: String,
    pub elapsed_seconds: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskListSummary {
    pub task_list_id: String,
    pub task_list_name: String,
    pub total_seconds: i64,
    pub total_minutes: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsSummary {
    pub total_seconds: i64,
    pub total_minutes: i64,
    /// direct_complete (duration=0) を除いたセッション数 (spec §4.3)
    pub total_sessions: i64,
    pub running_seconds: i64,
    pub paused_task_count: i64,
    pub by_task_list: Vec<TaskListSummary>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TodayDashboard {
    pub date_text: String,
    pub active_session: Option<ActiveSessionView>,
    pub task_groups: Vec<TaskGroup>,
    pub analytics: AnalyticsSummary,
}

/// フロントからタスクを特定するための参照。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRef {
    pub task_list_id: String,
    pub task_id: String,
}

/// タスクをその場で追加する際の、追加先タスクリストの選択肢。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskListOption {
    pub task_list_id: String,
    pub task_list_name: String,
}

/// タスク削除の取り消し (undo) 結果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreTaskResult {
    /// 復元できたか。
    pub restored: bool,
    /// Google 側で既に削除済みだったため、同じ内容の新規ローカルタスクとして
    /// 作り直したか (作り直した場合、元のタスクとは task_id が変わる)。
    pub recreated: bool,
}

/// DB から読んだ work_logs の 1 行 (状態導出・集計に使う最小限)。
#[derive(Debug, Clone)]
pub struct WorkLogRow {
    pub task_list_id: String,
    pub task_list_name: String,
    pub task_id: String,
    pub action_type: String,
    pub end_time: String,
    pub duration_seconds: i64,
    pub end_reason: String,
}

#[derive(Debug, Clone)]
pub struct ActiveSessionRow {
    pub task_list_id: String,
    pub task_list_name: String,
    pub task_id: String,
    pub task_title: String,
    pub start_at: String,
}
