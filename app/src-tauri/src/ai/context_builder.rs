//! 作戦生成の入力コンテキストを SQLite から収集する (AI 拡張仕様 §3.2)。
//! 送信するのは集計値と直近分のみ (生ログ全件は送らない — 仕様 §6)。

use chrono::Datelike;
use rusqlite::Connection;

use crate::db::repos;
use crate::domain::{status, time};

/// タスクごとの実績サマリ。
struct TaskStats {
    session_count: i64,
    avg_minutes: i64,
    last_worked: Option<String>,
}

fn task_stats(conn: &Connection, task_list_id: &str, task_id: &str) -> TaskStats {
    conn.query_row(
        "SELECT COUNT(*), COALESCE(AVG(duration_seconds), 0), MAX(log_date)
         FROM work_logs
         WHERE task_list_id = ?1 AND task_id = ?2
           AND NOT (duration_seconds = 0 AND end_reason = 'direct_complete')",
        rusqlite::params![task_list_id, task_id],
        |row| {
            Ok(TaskStats {
                session_count: row.get(0)?,
                avg_minutes: time::ceil_minutes(row.get::<_, f64>(1)? as i64),
                last_worked: row.get(2)?,
            })
        },
    )
    .unwrap_or(TaskStats {
        session_count: 0,
        avg_minutes: 0,
        last_worked: None,
    })
}

/// 日別実働 (通常タスク / null タスク別) を過去 N 日分。
fn daily_totals(conn: &Connection, days: i64) -> Vec<(String, i64, i64)> {
    let since = time::jst_date_text(&(time::now_utc() - chrono::Duration::days(days)));
    let mut stmt = match conn.prepare(
        "SELECT log_date,
                SUM(CASE WHEN task_id != 'null' THEN duration_seconds ELSE 0 END),
                SUM(CASE WHEN task_id = 'null' THEN duration_seconds ELSE 0 END)
         FROM work_logs WHERE log_date >= ?1 GROUP BY log_date ORDER BY log_date",
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    stmt.query_map(rusqlite::params![since], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })
    .map(|rows| rows.flatten().collect())
    .unwrap_or_default()
}

/// 作戦生成のユーザーペイロードを組み立てる。
pub fn build_payload(conn: &Connection, note: &str) -> Result<String, String> {
    let now = time::now_utc();
    let today = time::today_jst();
    let jst_now = now.with_timezone(&time::TIMEZONE);
    let weekday = ["月", "火", "水", "木", "金", "土", "日"]
        [jst_now.weekday().num_days_from_monday() as usize];

    let mut out = String::new();
    out.push_str(&format!(
        "# 現在\n{} ({}) {}\n\n",
        today,
        weekday,
        jst_now.format("%H:%M")
    ));

    if !note.trim().is_empty() {
        out.push_str(&format!("# 今日の状態メモ (ユーザー入力)\n{}\n\n", note.trim()));
    }

    // 今日のタスク (実績・状態込み)
    let logs_today = repos::fetch_logs_by_date(conn, &today).map_err(|e| e.to_string())?;
    let active = repos::get_active_session(conn).map_err(|e| e.to_string())?;
    let stats_today = status::build_today_stats(&logs_today, active.as_ref(), now.timestamp_millis());

    let open_tasks = repos::fetch_open_tasks(conn).map_err(|e| e.to_string())?;
    let (today_tasks, candidates): (Vec<_>, Vec<_>) = open_tasks
        .into_iter()
        .partition(|t| time::is_due_today(&t.due));

    out.push_str("# 今日期限のタスク\n");
    if today_tasks.is_empty() {
        out.push_str("(なし)\n");
    }
    for t in &today_tasks {
        let (secs, app_status, _) = status::lookup(&stats_today, &t.task_list_id, &t.id);
        let s = task_stats(conn, &t.task_list_id, &t.id);
        out.push_str(&format!(
            "- [{}] {} (taskListId={}, taskId={}) 状態={:?} 本日実績={}分 過去平均={}分/回×{}回{}{}\n",
            t.task_list_title,
            t.title,
            t.task_list_id,
            t.id,
            app_status,
            time::ceil_minutes(secs),
            s.avg_minutes,
            s.session_count,
            s.last_worked
                .as_deref()
                .map(|d| format!(" 最終作業={d}"))
                .unwrap_or_default(),
            if t.notes.is_empty() {
                String::new()
            } else {
                format!(" メモ: {}", truncate(&t.notes, 100))
            },
        ));
    }
    out.push('\n');

    // 候補タスク: 期限が近い順 (期限なしは後ろ)、上位 30 件。停滞情報つき
    let mut sorted = candidates;
    sorted.sort_by(|a, b| match (&a.due, &b.due) {
        (Some(x), Some(y)) => x.cmp(y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
    let total_candidates = sorted.len();
    out.push_str(&format!("# 候補タスク (今日期限でないもの、全{total_candidates}件中上位30件)\n"));
    for t in sorted.iter().take(30) {
        let s = task_stats(conn, &t.task_list_id, &t.id);
        let stalled = s
            .last_worked
            .as_deref()
            .map(|d| {
                time::parse_iso(&format!("{d}T00:00:00Z"))
                    .map(|dt| (now - dt).num_days())
                    .unwrap_or(0)
            })
            .filter(|days| *days >= 14 && s.session_count > 0);
        out.push_str(&format!(
            "- [{}] {} (taskListId={}, taskId={}) 期限={}{}{}{}\n",
            t.task_list_title,
            t.title,
            t.task_list_id,
            t.id,
            t.due.as_deref().map(|d| &d[0..10.min(d.len())]).unwrap_or("なし"),
            if s.session_count > 0 {
                format!(" 過去平均={}分/回×{}回", s.avg_minutes, s.session_count)
            } else {
                String::new()
            },
            stalled
                .map(|d| format!(" ★停滞{d}日"))
                .unwrap_or_default(),
            if t.notes.is_empty() {
                String::new()
            } else {
                format!(" メモ: {}", truncate(&t.notes, 80))
            },
        ));
    }
    out.push('\n');

    // 実績ペース (過去7日 + 平日/休日平均)
    out.push_str("# 実績ペース (過去7日、分)\n");
    let totals7 = daily_totals(conn, 7);
    if totals7.is_empty() {
        out.push_str("(記録なし)\n");
    }
    for (date, work, unassigned) in &totals7 {
        out.push_str(&format!(
            "- {}: タスク作業 {}分 / 未割当(null) {}分\n",
            date,
            time::ceil_minutes(*work),
            time::ceil_minutes(*unassigned)
        ));
    }
    let totals30 = daily_totals(conn, 30);
    if !totals30.is_empty() {
        let (mut wd, mut wd_n, mut we, mut we_n) = (0i64, 0i64, 0i64, 0i64);
        for (date, work, _) in &totals30 {
            if let Ok(d) = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d") {
                if d.weekday().num_days_from_monday() < 5 {
                    wd += work;
                    wd_n += 1;
                } else {
                    we += work;
                    we_n += 1;
                }
            }
        }
        out.push_str(&format!(
            "過去30日平均: 平日 {}分/日, 休日 {}分/日\n\n",
            if wd_n > 0 { time::ceil_minutes(wd / wd_n) } else { 0 },
            if we_n > 0 { time::ceil_minutes(we / we_n) } else { 0 },
        ));
    }

    // 昨日の実績
    let yesterday = time::jst_date_text(&(now - chrono::Duration::days(1)));
    let logs_y = repos::fetch_logs_by_date(conn, &yesterday).map_err(|e| e.to_string())?;
    out.push_str(&format!("# 昨日 ({yesterday}) の記録\n"));
    if logs_y.is_empty() {
        out.push_str("(なし)\n");
    }
    for log in logs_y.iter().filter(|l| l.task_id != "null") {
        out.push_str(&format!(
            "- {} {}分 ({})\n",
            if log.action_type == "completed" { "完了" } else { "中断" },
            time::ceil_minutes(log.duration_seconds),
            log.task_list_name,
        ));
    }

    out.push_str("\n上記をもとに、今日の作戦を JSON で出力せよ。");
    Ok(out)
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_string()
    } else {
        format!("{}…", text.chars().take(max_chars).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_contains_sections_and_task_ids() {
        let conn = crate::db::open_in_memory().unwrap();
        repos::seed_sample_data(&conn).unwrap();

        let payload = build_payload(&conn, "午後から外出").unwrap();
        assert!(payload.contains("# 現在"));
        assert!(payload.contains("午後から外出"));
        assert!(payload.contains("# 今日期限のタスク"));
        assert!(payload.contains("taskId=sample-task-1"));
        assert!(payload.contains("# 候補タスク"));
        assert!(payload.contains("# 実績ペース"));
        // メモの内容がペイロードに含まれる (送信内容の確認)
        assert!(payload.contains("図3を差し替える"));
    }
}
