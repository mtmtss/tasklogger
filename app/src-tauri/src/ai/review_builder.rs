//! 日次レビューの入力コンテキストを SQLite から収集する (AI 拡張仕様 §10 第2弾)。
//! 「今朝立てた作戦 (daily_plans)」と「実際にやったこと (work_logs)」を突合する。

use rusqlite::Connection;

use crate::ai::plan_store;
use crate::db::repos;
use crate::domain::time;

/// 対象日 (省略時は今日) のレビュー用ペイロードを組み立てる。
pub fn build_payload(conn: &Connection, date: &str) -> Result<String, String> {
    let mut out = String::new();
    out.push_str(&format!("# 振り返り対象日\n{date}\n\n"));

    // 今朝の作戦 (立てていれば)
    match plan_store::latest_plan_today(conn)? {
        Some(stored) if stored.plan_date == date => {
            out.push_str("# 今朝立てた作戦\n");
            if !stored.input_note.is_empty() {
                out.push_str(&format!("(当日メモ: {})\n", stored.input_note));
            }
            let plan = &stored.plan;
            for (label, key) in [
                ("必ずやる", "must_do"),
                ("できれば", "if_possible"),
                ("5分", "five_minute"),
            ] {
                if let Some(items) = plan[key].as_array() {
                    for item in items {
                        out.push_str(&format!(
                            "- [{}] {} (見積 {}分)\n",
                            label,
                            item["title"].as_str().unwrap_or(""),
                            item["estimatedMinutes"].as_i64().unwrap_or(0),
                        ));
                    }
                }
            }
            out.push('\n');
        }
        _ => out.push_str("# 今朝立てた作戦\n(作戦は立てられていない)\n\n"),
    }

    // 実際の記録 (work_logs)
    let logs = repos::fetch_logs_by_range(conn, date, date).map_err(|e| e.to_string())?;
    let mut total = 0i64;
    let mut null_total = 0i64;
    let mut completed = 0i64;
    let mut interrupted = 0i64;
    let mut idle_or_sleep = 0i64;

    out.push_str("# 実際の記録\n");
    if logs.is_empty() {
        out.push_str("(記録なし)\n");
    }
    for log in &logs {
        total += log.duration_seconds;
        if log.task_id == "null" {
            null_total += log.duration_seconds;
            continue;
        }
        match log.action_type.as_str() {
            "completed" => completed += 1,
            _ => interrupted += 1,
        }
        if log.end_reason == "idle" || log.end_reason == "sleep" {
            idle_or_sleep += 1;
        }
        out.push_str(&format!(
            "- {} {}分 「{}」({}){}{}\n",
            if log.action_type == "completed" { "完了" } else { "中断" },
            time::ceil_minutes(log.duration_seconds),
            log.task_title,
            log.task_list_name,
            match log.end_reason.as_str() {
                "idle" => " ※無操作で自動中断",
                "sleep" => " ※スリープで自動中断",
                "recovery" => " ※異常終了で自動記録",
                _ => "",
            },
            if log.memo.is_empty() {
                String::new()
            } else {
                format!(" メモ: {}", log.memo)
            },
        ));
    }

    out.push_str(&format!(
        "\n# 集計\n- タスク作業 合計 {}分 / 未割当(null) {}分\n- 完了 {}件 / 中断 {}件 (うち無操作・スリープ由来 {}件)\n\n",
        time::ceil_minutes(total - null_total),
        time::ceil_minutes(null_total),
        completed,
        interrupted,
        idle_or_sleep,
    ));

    out.push_str(
        "上記をもとに、今日の振り返りを JSON で出力せよ。\
         計画と実績の乖離があれば理由を推測し、責めずに次につなげること。\
         null(未割当)時間が多い場合はタスク化の習慣づけを促してもよい。",
    );
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repos::NewWorkLog;

    #[test]
    fn payload_reflects_logs_and_missing_plan() {
        let conn = crate::db::open_in_memory().unwrap();
        repos::seed_sample_data(&conn).unwrap();
        let today = time::today_jst();
        let now = time::to_iso(&time::now_utc());

        repos::append_work_log(
            &conn,
            &NewWorkLog {
                task_list_id: "sample-list-1",
                task_list_name: "研究",
                task_id: "sample-task-1",
                task_title: "論文の実験セクション",
                action_type: "completed",
                start_time: now.clone(),
                end_time: now.clone(),
                duration_seconds: 1800,
                memo: "図3まで完了",
                end_reason: "user",
            },
        )
        .unwrap();
        // null 時間
        repos::append_work_log(
            &conn,
            &NewWorkLog {
                task_list_id: "null",
                task_list_name: "null",
                task_id: "null",
                task_title: "null",
                action_type: "paused",
                start_time: now.clone(),
                end_time: now,
                duration_seconds: 600,
                memo: "",
                end_reason: "idle",
            },
        )
        .unwrap();

        let payload = build_payload(&conn, &today).unwrap();
        assert!(payload.contains("# 今朝立てた作戦"));
        assert!(payload.contains("作戦は立てられていない"));
        assert!(payload.contains("完了 30分 「論文の実験セクション」"));
        assert!(payload.contains("図3まで完了"));
        assert!(payload.contains("未割当(null) 10分"));
        assert!(payload.contains("完了 1件"));
    }
}
