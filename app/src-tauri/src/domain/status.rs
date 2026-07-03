use std::collections::HashMap;

use super::models::{ActiveSessionRow, AppStatus, WorkLogRow};
use super::time::parse_iso;

fn task_key(task_list_id: &str, task_id: &str) -> String {
    format!("{}::{}", task_list_id, task_id)
}

#[derive(Debug, Clone, Default)]
pub struct TaskStats {
    pub today_duration_seconds: i64,
    pub app_status_from_logs: Option<(i64, AppStatus)>, // (最新ログの時刻ms, 状態)
    pub session_count: i64,
}

/// GAS 版 buildTodayTaskStats_ 相当。
/// 今日のログ + アクティブセッションから、タスクごとの累計秒と導出状態を作る (spec §4.1)。
pub fn build_today_stats(
    logs: &[WorkLogRow],
    active: Option<&ActiveSessionRow>,
    now_ms: i64,
) -> HashMap<String, (i64, AppStatus, i64)> {
    let mut acc: HashMap<String, TaskStats> = HashMap::new();

    for log in logs {
        let key = task_key(&log.task_list_id, &log.task_id);
        let entry = acc.entry(key).or_default();
        entry.today_duration_seconds += log.duration_seconds;

        // direct_complete (duration=0) は作業回数に数えない (spec §4.3)
        if !(log.duration_seconds == 0 && log.end_reason == "direct_complete") {
            entry.session_count += 1;
        }

        let log_ms = parse_iso(&log.end_time)
            .map(|dt| dt.timestamp_millis())
            .unwrap_or(0);
        let status = if log.action_type == "completed" {
            AppStatus::Completed
        } else {
            AppStatus::Paused
        };
        // 同時刻なら後から挿入された行 (走査順で後) を最新とみなす
        match entry.app_status_from_logs {
            Some((latest, _)) if log_ms < latest => {}
            _ => entry.app_status_from_logs = Some((log_ms, status)),
        }
    }

    let mut result: HashMap<String, (i64, AppStatus, i64)> = acc
        .into_iter()
        .map(|(key, stats)| {
            let status = stats
                .app_status_from_logs
                .map(|(_, s)| s)
                .unwrap_or(AppStatus::NotStarted);
            (
                key,
                (stats.today_duration_seconds, status, stats.session_count),
            )
        })
        .collect();

    // アクティブセッションは最優先で running + 経過秒を加算
    if let Some(session) = active {
        let key = task_key(&session.task_list_id, &session.task_id);
        let elapsed = elapsed_seconds(&session.start_at, now_ms);
        let entry = result.entry(key).or_insert((0, AppStatus::Running, 0));
        entry.0 += elapsed;
        entry.1 = AppStatus::Running;
    }

    result
}

pub fn elapsed_seconds(start_at_iso: &str, now_ms: i64) -> i64 {
    parse_iso(start_at_iso)
        .map(|dt| ((now_ms - dt.timestamp_millis()) / 1000).max(0))
        .unwrap_or(0)
}

pub fn lookup(
    stats: &HashMap<String, (i64, AppStatus, i64)>,
    task_list_id: &str,
    task_id: &str,
) -> (i64, AppStatus, i64) {
    stats
        .get(&task_key(task_list_id, task_id))
        .cloned()
        .unwrap_or((0, AppStatus::NotStarted, 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log(
        task_id: &str,
        action: &str,
        end_time: &str,
        duration: i64,
        end_reason: &str,
    ) -> WorkLogRow {
        WorkLogRow {
            task_list_id: "L1".into(),
            task_list_name: "list".into(),
            task_id: task_id.into(),
            action_type: action.into(),
            end_time: end_time.into(),
            duration_seconds: duration,
            end_reason: end_reason.into(),
        }
    }

    #[test]
    fn not_started_when_no_logs() {
        let stats = build_today_stats(&[], None, 0);
        let (secs, status, sessions) = lookup(&stats, "L1", "T1");
        assert_eq!(secs, 0);
        assert_eq!(status, AppStatus::NotStarted);
        assert_eq!(sessions, 0);
    }

    #[test]
    fn paused_after_pause_log() {
        let logs = vec![log("T1", "paused", "2026-07-04T01:00:00Z", 600, "user")];
        let stats = build_today_stats(&logs, None, 0);
        let (secs, status, sessions) = lookup(&stats, "L1", "T1");
        assert_eq!(secs, 600);
        assert_eq!(status, AppStatus::Paused);
        assert_eq!(sessions, 1);
    }

    #[test]
    fn completed_wins_when_latest() {
        let logs = vec![
            log("T1", "paused", "2026-07-04T01:00:00Z", 600, "user"),
            log("T1", "completed", "2026-07-04T02:00:00Z", 300, "user"),
        ];
        let stats = build_today_stats(&logs, None, 0);
        let (secs, status, _) = lookup(&stats, "L1", "T1");
        assert_eq!(secs, 900);
        assert_eq!(status, AppStatus::Completed);
    }

    #[test]
    fn direct_complete_sets_completed_but_not_counted_as_session() {
        // 中断中タスクの直接完了 (spec §4.3): duration=0 の completed 行
        let logs = vec![
            log("T1", "paused", "2026-07-04T01:00:00Z", 600, "user"),
            log(
                "T1",
                "completed",
                "2026-07-04T02:00:00Z",
                0,
                "direct_complete",
            ),
        ];
        let stats = build_today_stats(&logs, None, 0);
        let (secs, status, sessions) = lookup(&stats, "L1", "T1");
        assert_eq!(secs, 600, "合計時間は変わらない");
        assert_eq!(status, AppStatus::Completed);
        assert_eq!(sessions, 1, "direct_complete は作業回数に数えない");
    }

    #[test]
    fn sleep_pause_is_a_normal_paused_log() {
        let logs = vec![log("T1", "paused", "2026-07-04T01:00:00Z", 120, "sleep")];
        let stats = build_today_stats(&logs, None, 0);
        let (_, status, sessions) = lookup(&stats, "L1", "T1");
        assert_eq!(status, AppStatus::Paused);
        assert_eq!(sessions, 1);
    }

    #[test]
    fn running_overrides_logs_and_adds_elapsed() {
        let logs = vec![log("T1", "paused", "2026-07-04T01:00:00Z", 600, "user")];
        let session = ActiveSessionRow {
            task_list_id: "L1".into(),
            task_list_name: "list".into(),
            task_id: "T1".into(),
            task_title: "task".into(),
            start_at: "2026-07-04T03:00:00Z".into(),
        };
        // now = 03:05:00 → 経過 300 秒
        let now_ms = parse_iso("2026-07-04T03:05:00Z").unwrap().timestamp_millis();
        let stats = build_today_stats(&logs, Some(&session), now_ms);
        let (secs, status, _) = lookup(&stats, "L1", "T1");
        assert_eq!(secs, 900);
        assert_eq!(status, AppStatus::Running);
    }
}
