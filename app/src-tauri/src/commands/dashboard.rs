use std::collections::BTreeMap;

use tauri::State;

use super::{build_session_view, db_err, CmdResult};
use crate::db::repos;
use crate::domain::models::{
    AnalyticsSummary, AppStatus, TaskGroup, TaskItem, TaskListSummary, TodayDashboard,
};
use crate::domain::{status, time};
use crate::state::AppState;

/// 今日ダッシュボード (spec §5.1, §5.4)。ローカル DB のみ参照するため即時 (spec §6.5)。
#[tauri::command]
pub fn get_today_dashboard(state: State<'_, AppState>) -> CmdResult<TodayDashboard> {
    let conn = state.db.lock().unwrap();
    let today = time::today_jst();
    let now_ms = time::now_utc().timestamp_millis();

    let logs = repos::fetch_logs_by_date(&conn, &today).map_err(db_err)?;
    let active = repos::get_active_session(&conn).map_err(db_err)?;
    let stats = status::build_today_stats(&logs, active.as_ref(), now_ms);

    // 今日期限の未完了タスク (Google 側 status='completed' は除外済み)
    let open_tasks = repos::fetch_open_tasks(&conn).map_err(db_err)?;
    let today_tasks: Vec<_> = open_tasks
        .into_iter()
        .filter(|t| time::is_due_today(&t.due))
        .collect();

    let task_groups = group_tasks(today_tasks, &stats);

    // 今日の集計
    let mut total_seconds = 0i64;
    let mut total_sessions = 0i64;
    let mut by_list: BTreeMap<String, TaskListSummary> = BTreeMap::new();
    for log in &logs {
        total_seconds += log.duration_seconds;
        if !(log.duration_seconds == 0 && log.end_reason == "direct_complete") {
            total_sessions += 1;
        }
        let entry = by_list
            .entry(log.task_list_id.clone())
            .or_insert_with(|| TaskListSummary {
                task_list_id: log.task_list_id.clone(),
                task_list_name: log.task_list_name.clone(),
                total_seconds: 0,
                total_minutes: 0,
            });
        entry.total_seconds += log.duration_seconds;
    }

    let mut running_seconds = 0i64;
    if let Some(session) = &active {
        running_seconds = status::elapsed_seconds(&session.start_at, now_ms);
        total_seconds += running_seconds;
        let entry = by_list
            .entry(session.task_list_id.clone())
            .or_insert_with(|| TaskListSummary {
                task_list_id: session.task_list_id.clone(),
                task_list_name: session.task_list_name.clone(),
                total_seconds: 0,
                total_minutes: 0,
            });
        entry.total_seconds += running_seconds;
    }

    let mut by_task_list: Vec<TaskListSummary> = by_list
        .into_values()
        .map(|mut item| {
            item.total_minutes = time::ceil_minutes(item.total_seconds);
            item
        })
        .collect();
    by_task_list.sort_by(|a, b| b.total_seconds.cmp(&a.total_seconds));

    let paused_task_count = task_groups
        .iter()
        .flat_map(|g| &g.tasks)
        .filter(|t| t.app_status == AppStatus::Paused)
        .count() as i64;

    Ok(TodayDashboard {
        date_text: today,
        active_session: build_session_view(&conn)?,
        task_groups,
        analytics: AnalyticsSummary {
            total_seconds,
            total_minutes: time::ceil_minutes(total_seconds),
            total_sessions,
            running_seconds,
            paused_task_count,
            by_task_list,
        },
    })
}

/// タスクリスト一覧 (Inbox の登録先選択・クイック追加用, AI 拡張仕様 §13.5)。
#[tauri::command]
pub fn get_task_lists(state: State<'_, AppState>) -> CmdResult<Vec<TaskListOption>> {
    let conn = state.db.lock().unwrap();
    let rows = repos::fetch_task_lists(&conn).map_err(db_err)?;
    Ok(rows
        .into_iter()
        .map(|l| TaskListOption { id: l.id, title: l.title })
        .collect())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskListOption {
    pub id: String,
    pub title: String,
}

/// 候補タスク (spec §5.3): due が今日でない or due なしの未完了タスク。
#[tauri::command]
pub fn get_candidates(state: State<'_, AppState>) -> CmdResult<Vec<TaskGroup>> {
    let conn = state.db.lock().unwrap();
    let open_tasks = repos::fetch_open_tasks(&conn).map_err(db_err)?;
    let candidates: Vec<_> = open_tasks
        .into_iter()
        .filter(|t| !time::is_due_today(&t.due))
        .collect();
    Ok(group_tasks(candidates, &Default::default()))
}

fn group_tasks(
    rows: Vec<repos::TaskRow>,
    stats: &std::collections::HashMap<String, (i64, AppStatus, i64)>,
) -> Vec<TaskGroup> {
    let mut groups: BTreeMap<(String, String), Vec<TaskItem>> = BTreeMap::new();
    for row in rows {
        let (secs, app_status, _) = status::lookup(stats, &row.task_list_id, &row.id);
        groups
            .entry((row.task_list_title.clone(), row.task_list_id.clone()))
            .or_default()
            .push(TaskItem {
                task_list_id: row.task_list_id,
                task_list_name: row.task_list_title,
                task_id: row.id,
                title: row.title,
                notes: row.notes,
                due: row.due,
                status: row.status,
                app_status,
                today_duration_seconds: secs,
                today_duration_minutes: time::ceil_minutes(secs),
            });
    }
    groups
        .into_iter()
        .map(|((name, id), tasks)| TaskGroup {
            task_list_id: id,
            task_list_name: name,
            tasks,
        })
        .collect()
}
