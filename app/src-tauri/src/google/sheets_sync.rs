//! 作業ログの Google Sheets 双方向同期 (spec §6.6)。
//!
//! - 正本はローカル SQLite。Sheets は複数デバイス間の合流点 (append-only)
//! - logId の差分で pull (リモートにあってローカルにない行を取込) と
//!   push (ローカルにあってリモートにない行を追記) を行う
//! - すべて同期ワーカースレッドで実行され、UI 操作を一切ブロックしない
//! - シートは GAS 版 WorkLogs と互換 (14 列 + endReason/source 拡張 2 列)

use std::collections::{HashMap, HashSet};

use tauri::{Emitter, Manager};

use super::sheets_api;
use crate::commands::import_export::{normalize_date_text, parse_datetime};
use crate::db::repos::{self, FullLogRow};
use crate::domain::time;
use crate::state::AppState;

pub const SHEET_NAME: &str = "WorkLogs";
const SPREADSHEET_TITLE: &str = "TaskLogger Logs";

const HEADERS: [&str; 16] = [
    "logId",
    "userId",
    "taskListId",
    "taskListName",
    "taskId",
    "taskTitle",
    "actionType",
    "startTime",
    "endTime",
    "durationSeconds",
    "durationMinutes",
    "logDate",
    "memo",
    "createdAt",
    "endReason",
    "source",
];

/// 有効時のみ呼ばれる前提。1 サイクル分の pull → push を行う。
pub fn perform_sheet_sync(app: &tauri::AppHandle, token: &str) -> Result<(), String> {
    let spreadsheet_id = ensure_spreadsheet(app, token)?;
    let rows = sheets_api::values_get(token, &spreadsheet_id, &format!("{SHEET_NAME}!A1:P"))
        .map_err(|e| e.to_string())?;

    // ヘッダの検証・整備
    let header = match rows.first() {
        Some(h) if !h.is_empty() => h.clone(),
        _ => {
            // 空シート: ヘッダを書いて全ローカル行を push
            sheets_api::values_update(
                token,
                &spreadsheet_id,
                &format!("{SHEET_NAME}!A1:P1"),
                &[HEADERS.iter().map(|s| s.to_string()).collect()],
            )
            .map_err(|e| e.to_string())?;
            HEADERS.iter().map(|s| s.to_string()).collect()
        }
    };
    validate_header(&header)?;
    let header = extend_header_if_needed(token, &spreadsheet_id, header)?;
    let index: HashMap<&str, usize> = header
        .iter()
        .enumerate()
        .map(|(i, h)| (h.trim(), i))
        .collect();

    // ---- Pull: リモートにあってローカルにない行を取り込む ----
    let mut remote_ids: HashSet<String> = HashSet::new();
    let mut pulled = 0usize;
    {
        let state = app.state::<AppState>();
        for row in rows.iter().skip(1) {
            let get = |name: &str| -> String {
                index
                    .get(name)
                    .and_then(|&i| row.get(i))
                    .map(|s| s.trim().to_string())
                    .unwrap_or_default()
            };
            let log_id = get("logId");
            if log_id.is_empty() {
                continue;
            }
            remote_ids.insert(log_id.clone());

            let start_raw = get("startTime");
            let end_raw = get("endTime");
            let start = parse_datetime(&start_raw);
            let end = parse_datetime(&end_raw);
            let mut duration_seconds = parse_int(&get("durationSeconds"));
            if duration_seconds <= 0 {
                duration_seconds = match (&start, &end) {
                    (Some(s), Some(e)) => ((*e - *s).num_seconds()).max(0),
                    _ => parse_int(&get("durationMinutes")) * 60,
                };
            }
            let log_date = normalize_date_text(&get("logDate"))
                .or_else(|| start.as_ref().map(time::jst_date_text))
                .unwrap_or_default();

            let record = FullLogRow {
                log_id,
                user_id: get("userId"),
                task_list_id: get("taskListId"),
                task_list_name: get("taskListName"),
                task_id: get("taskId"),
                task_title: get("taskTitle"),
                action_type: if get("actionType") == "completed" {
                    "completed".into()
                } else {
                    "paused".into()
                },
                start_time: start.map(|d| time::to_iso(&d)).unwrap_or(start_raw),
                end_time: end.map(|d| time::to_iso(&d)).unwrap_or(end_raw),
                duration_seconds,
                duration_minutes: time::ceil_minutes(duration_seconds),
                log_date,
                memo: get("memo"),
                created_at: get("createdAt"),
                end_reason: {
                    let er = get("endReason");
                    if er.is_empty() { "user".into() } else { er }
                },
                source: "sheet_pull".into(),
            };

            let conn = state.db.lock().unwrap();
            if repos::insert_imported_log(&conn, &record).map_err(|e| e.to_string())? {
                pulled += 1;
            }
        }
    }

    // ---- Push: ローカルにあってリモートにない行を追記する ----
    let to_push: Vec<Vec<String>> = {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap();
        repos::fetch_logs_by_range(&conn, "0000-01-01", "9999-12-31")
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|log| !remote_ids.contains(&log.log_id))
            .map(to_sheet_row)
            .collect()
    };
    let pushed = to_push.len();
    sheets_api::values_append(
        token,
        &spreadsheet_id,
        &format!("{SHEET_NAME}!A1:P1"),
        &to_push,
    )
    .map_err(|e| e.to_string())?;

    {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap();
        let _ = repos::set_setting(
            &conn,
            "last_sheet_sync_at",
            &time::to_iso(&time::now_utc()),
        );
    }
    if pulled > 0 {
        let _ = app.emit("tasks-changed", ()); // アーカイブ集計の再取得
    }
    let _ = app.emit("sync-status-changed", ());
    let _ = (pulled, pushed);
    Ok(())
}

/// 設定の spreadsheet ID を返す。未設定なら新規作成して保存する。
fn ensure_spreadsheet(app: &tauri::AppHandle, token: &str) -> Result<String, String> {
    let existing = {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap();
        repos::get_setting(&conn, "log_spreadsheet_id")
            .map_err(|e| e.to_string())?
            .filter(|v| !v.trim().is_empty())
    };

    if let Some(id) = existing {
        let id = id.trim().to_string();
        // WorkLogs シートが無ければ追加 (既存 GAS スプレッドシートには既にある)
        let titles = sheets_api::list_sheet_titles(token, &id).map_err(|e| e.to_string())?;
        if !titles.iter().any(|t| t == SHEET_NAME) {
            sheets_api::add_sheet(token, &id, SHEET_NAME).map_err(|e| e.to_string())?;
        }
        return Ok(id);
    }

    let id = sheets_api::create_spreadsheet(token, SPREADSHEET_TITLE, SHEET_NAME)
        .map_err(|e| e.to_string())?;
    let state = app.state::<AppState>();
    let conn = state.db.lock().unwrap();
    repos::set_setting(&conn, "log_spreadsheet_id", &id).map_err(|e| e.to_string())?;
    Ok(id)
}

/// GAS v2 互換の列順であることを確認する。旧 (legacy) スキーマの場合は
/// 列位置が異なり追記すると壊れるため、CSV インポートを案内して拒否する。
fn validate_header(header: &[String]) -> Result<(), String> {
    for (i, expected) in HEADERS.iter().take(14).enumerate() {
        match header.get(i) {
            Some(actual) if actual.trim() == *expected => {}
            _ => {
                return Err(format!(
                    "スプレッドシートの列構成が想定 (GAS 版 v2 スキーマ) と異なります \
                     (列 {} が '{}' ではありません)。旧形式のシートは設定ページの \
                     CSV インポートで取り込み、同期には新しいシートを使ってください。",
                    i + 1,
                    expected
                ));
            }
        }
    }
    Ok(())
}

/// GAS 版シート (14 列) には endReason / source 列を追加する。データ行は触らない。
fn extend_header_if_needed(
    token: &str,
    spreadsheet_id: &str,
    header: Vec<String>,
) -> Result<Vec<String>, String> {
    let has_extension = header.iter().any(|h| h.trim() == "endReason");
    if has_extension {
        return Ok(header);
    }
    sheets_api::values_update(
        token,
        spreadsheet_id,
        &format!("{SHEET_NAME}!O1:P1"),
        &[vec!["endReason".to_string(), "source".to_string()]],
    )
    .map_err(|e| e.to_string())?;
    let mut extended = header;
    while extended.len() < 14 {
        extended.push(String::new());
    }
    extended.truncate(14);
    extended.push("endReason".to_string());
    extended.push("source".to_string());
    Ok(extended)
}

fn to_sheet_row(log: FullLogRow) -> Vec<String> {
    vec![
        log.log_id,
        log.user_id,
        log.task_list_id,
        log.task_list_name,
        log.task_id,
        log.task_title,
        log.action_type,
        log.start_time,
        log.end_time,
        log.duration_seconds.to_string(),
        log.duration_minutes.to_string(),
        log.log_date,
        log.memo,
        log.created_at,
        log.end_reason,
        log.source,
    ]
}

/// "90" / "90.0" (UNFORMATTED_VALUE の数値セル) の両方を受け付ける。
fn parse_int(text: &str) -> i64 {
    text.parse::<i64>()
        .ok()
        .or_else(|| text.parse::<f64>().ok().map(|f| f.round() as i64))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_int_accepts_float_cells() {
        assert_eq!(parse_int("90"), 90);
        assert_eq!(parse_int("90.0"), 90);
        assert_eq!(parse_int(""), 0);
        assert_eq!(parse_int("abc"), 0);
    }

    #[test]
    fn validates_gas_v2_header() {
        let ok: Vec<String> = HEADERS.iter().map(|s| s.to_string()).collect();
        assert!(validate_header(&ok).is_ok());
        assert!(validate_header(&ok[0..14].to_vec()).is_ok(), "拡張列なしの GAS v2 も許容");

        let legacy: Vec<String> = [
            "logId", "date", "taskListId", "taskListName", "taskId", "taskTitle",
            "startAt", "endAt", "minutes", "action", "memo", "createdAt",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert!(validate_header(&legacy).is_err(), "legacy スキーマは拒否");
    }
}
