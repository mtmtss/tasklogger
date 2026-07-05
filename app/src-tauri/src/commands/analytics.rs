//! アーカイブ (期間分析, spec §5.5)。GAS 版 buildArchiveAnalyticsSummary_ の移植。

use std::collections::BTreeMap;

use chrono::NaiveDate;
use serde::Serialize;
use tauri::State;

use super::{db_err, CmdResult};
use crate::db::repos;
use crate::domain::time;
use crate::state::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DateSummary {
    pub date: String,
    pub total_seconds: i64,
    pub total_minutes: i64,
    pub session_count: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSummary {
    pub task_list_id: String,
    pub task_list_name: String,
    pub total_seconds: i64,
    pub total_minutes: i64,
    pub session_count: i64,
    pub completed_count: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSummary {
    pub task_list_id: String,
    pub task_list_name: String,
    pub task_id: String,
    pub task_title: String,
    pub total_seconds: i64,
    pub total_minutes: i64,
    pub session_count: i64,
    pub completed_count: i64,
    pub last_worked_date: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveAnalytics {
    pub start_date: String,
    pub end_date: String,
    pub total_seconds: i64,
    pub total_minutes: i64,
    pub total_sessions: i64,
    pub completed_sessions: i64,
    pub active_days: i64,
    pub average_minutes_per_day: i64,
    pub by_date: Vec<DateSummary>,
    pub by_task_list: Vec<ListSummary>,
    pub by_task: Vec<TaskSummary>,
}

#[tauri::command]
pub fn get_archive_analytics(
    state: State<'_, AppState>,
    start_date: String,
    end_date: String,
) -> CmdResult<ArchiveAnalytics> {
    let (start, end) = normalize_range(&start_date, &end_date)?;

    let logs = {
        let conn = state.db.lock().unwrap();
        repos::fetch_logs_by_range(&conn, &start, &end).map_err(db_err)?
    };

    let mut total_seconds = 0i64;
    let mut total_sessions = 0i64;
    let mut completed_sessions = 0i64;
    let mut by_date: BTreeMap<String, DateSummary> = BTreeMap::new();
    let mut by_list: BTreeMap<String, ListSummary> = BTreeMap::new();
    let mut by_task: BTreeMap<String, TaskSummary> = BTreeMap::new();

    for log in &logs {
        // direct_complete (duration=0) は作業回数に数えない (spec §4.3)
        let counts_as_session = !(log.duration_seconds == 0 && log.end_reason == "direct_complete");

        total_seconds += log.duration_seconds;
        if counts_as_session {
            total_sessions += 1;
        }
        if log.action_type == "completed" {
            completed_sessions += 1;
        }

        let date_entry = by_date
            .entry(log.log_date.clone())
            .or_insert_with(|| DateSummary {
                date: log.log_date.clone(),
                total_seconds: 0,
                total_minutes: 0,
                session_count: 0,
            });
        date_entry.total_seconds += log.duration_seconds;
        if counts_as_session {
            date_entry.session_count += 1;
        }

        let list_entry = by_list
            .entry(log.task_list_id.clone())
            .or_insert_with(|| ListSummary {
                task_list_id: log.task_list_id.clone(),
                task_list_name: log.task_list_name.clone(),
                total_seconds: 0,
                total_minutes: 0,
                session_count: 0,
                completed_count: 0,
            });
        list_entry.total_seconds += log.duration_seconds;
        if counts_as_session {
            list_entry.session_count += 1;
        }
        if log.action_type == "completed" {
            list_entry.completed_count += 1;
        }

        let task_key = format!("{}::{}", log.task_list_id, log.task_id);
        let task_entry = by_task.entry(task_key).or_insert_with(|| TaskSummary {
            task_list_id: log.task_list_id.clone(),
            task_list_name: log.task_list_name.clone(),
            task_id: log.task_id.clone(),
            task_title: log.task_title.clone(),
            total_seconds: 0,
            total_minutes: 0,
            session_count: 0,
            completed_count: 0,
            last_worked_date: log.log_date.clone(),
        });
        task_entry.total_seconds += log.duration_seconds;
        if counts_as_session {
            task_entry.session_count += 1;
        }
        if log.action_type == "completed" {
            task_entry.completed_count += 1;
        }
        if log.log_date > task_entry.last_worked_date {
            task_entry.last_worked_date = log.log_date.clone();
        }
    }

    // 期間内の全日付 (0 分の日も含める, spec §5.5)
    let by_date_items = build_date_range(&start, &end)?
        .into_iter()
        .map(|date| {
            let mut item = by_date.remove(&date).unwrap_or(DateSummary {
                date,
                total_seconds: 0,
                total_minutes: 0,
                session_count: 0,
            });
            item.total_minutes = time::ceil_minutes(item.total_seconds);
            item
        })
        .collect::<Vec<_>>();

    let mut by_task_list: Vec<ListSummary> = by_list
        .into_values()
        .map(|mut item| {
            item.total_minutes = time::ceil_minutes(item.total_seconds);
            item
        })
        .collect();
    by_task_list.sort_by(|a, b| b.total_seconds.cmp(&a.total_seconds));

    let mut by_task_items: Vec<TaskSummary> = by_task
        .into_values()
        .map(|mut item| {
            item.total_minutes = time::ceil_minutes(item.total_seconds);
            item
        })
        .collect();
    by_task_items.sort_by(|a, b| b.total_seconds.cmp(&a.total_seconds));

    let active_days = by_date_items.iter().filter(|d| d.total_seconds > 0).count() as i64;
    let day_count = by_date_items.len() as i64;

    Ok(ArchiveAnalytics {
        start_date: start,
        end_date: end,
        total_seconds,
        total_minutes: time::ceil_minutes(total_seconds),
        total_sessions,
        completed_sessions,
        active_days,
        average_minutes_per_day: if day_count == 0 {
            0
        } else {
            time::ceil_minutes(total_seconds) / day_count
        },
        by_date: by_date_items,
        by_task_list,
        by_task: by_task_items,
    })
}

fn normalize_range(start: &str, end: &str) -> CmdResult<(String, String)> {
    let parse = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d");
    let start_date = parse(start).map_err(|_| format!("不正な開始日: {start}"))?;
    let end_date = parse(end).map_err(|_| format!("不正な終了日: {end}"))?;
    if start_date <= end_date {
        Ok((start.to_string(), end.to_string()))
    } else {
        Ok((end.to_string(), start.to_string()))
    }
}

fn build_date_range(start: &str, end: &str) -> CmdResult<Vec<String>> {
    let mut cursor =
        NaiveDate::parse_from_str(start, "%Y-%m-%d").map_err(|e| e.to_string())?;
    let end = NaiveDate::parse_from_str(end, "%Y-%m-%d").map_err(|e| e.to_string())?;
    let mut dates = Vec::new();
    while cursor <= end {
        dates.push(cursor.format("%Y-%m-%d").to_string());
        cursor = cursor
            .succ_opt()
            .ok_or("日付範囲が不正です")?;
        if dates.len() > 3700 {
            return Err("期間が長すぎます (最大 10 年)".into());
        }
    }
    Ok(dates)
}
