//! CSV エクスポート / GAS 旧データインポート (spec §5.6)。

use std::collections::HashMap;

use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use serde::Serialize;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

use super::{db_err, emit_tasks_changed, CmdResult};
use crate::db::repos::{self, FullLogRow};
use crate::domain::time;
use crate::state::AppState;

/// GAS WorkLogs 互換の列順 + 拡張 2 列 (spec §3.2 互換表)。
const CSV_HEADERS: [&str; 16] = [
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

/// 期間を指定して work_logs を CSV 保存する。保存ダイアログでキャンセルしたら None。
#[tauri::command]
pub async fn export_csv(
    app: tauri::AppHandle,
    start_date: String,
    end_date: String,
) -> CmdResult<Option<String>> {
    let default_name = format!("tasklogger_logs_{start_date}_{end_date}.csv");

    let path = tauri::async_runtime::spawn_blocking({
        let app = app.clone();
        move || {
            app.dialog()
                .file()
                .set_file_name(&default_name)
                .add_filter("CSV", &["csv"])
                .blocking_save_file()
        }
    })
    .await
    .map_err(|e| e.to_string())?;

    let Some(path) = path else { return Ok(None) };
    let path = path
        .into_path()
        .map_err(|e| format!("保存先の解決に失敗しました: {e}"))?;

    let logs = {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap();
        repos::fetch_logs_by_range(&conn, &start_date, &end_date).map_err(db_err)?
    };

    let mut writer = csv::Writer::from_path(&path)
        .map_err(|e| format!("CSV を書き込めません: {e}"))?;
    writer.write_record(CSV_HEADERS).map_err(|e| e.to_string())?;
    for log in &logs {
        writer
            .write_record([
                log.log_id.as_str(),
                log.user_id.as_str(),
                log.task_list_id.as_str(),
                log.task_list_name.as_str(),
                log.task_id.as_str(),
                log.task_title.as_str(),
                log.action_type.as_str(),
                log.start_time.as_str(),
                log.end_time.as_str(),
                &log.duration_seconds.to_string(),
                &log.duration_minutes.to_string(),
                log.log_date.as_str(),
                log.memo.as_str(),
                log.created_at.as_str(),
                log.end_reason.as_str(),
                log.source.as_str(),
            ])
            .map_err(|e| e.to_string())?;
    }
    writer.flush().map_err(|e| e.to_string())?;

    Ok(Some(path.display().to_string()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub imported: i64,
    pub skipped: i64,
}

/// GAS 版 WorkLogs シートの CSV を取り込む。
/// 新旧スキーマをヘッダ名で自動判別し、logId 重複はスキップ (spec §5.6)。
#[tauri::command]
pub async fn import_gas_csv(app: tauri::AppHandle) -> CmdResult<Option<ImportResult>> {
    let path = tauri::async_runtime::spawn_blocking({
        let app = app.clone();
        move || {
            app.dialog()
                .file()
                .add_filter("CSV", &["csv"])
                .blocking_pick_file()
        }
    })
    .await
    .map_err(|e| e.to_string())?;

    let Some(path) = path else { return Ok(None) };
    let path = path
        .into_path()
        .map_err(|e| format!("ファイルの解決に失敗しました: {e}"))?;

    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_path(&path)
        .map_err(|e| format!("CSV を読み込めません: {e}"))?;

    let headers: Vec<String> = reader
        .headers()
        .map_err(|e| e.to_string())?
        .iter()
        .map(|h| h.trim().to_string())
        .collect();
    let index: HashMap<&str, usize> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| (h.as_str(), i))
        .collect();

    if !index.contains_key("logId") {
        return Err("logId 列が見つかりません。GAS の WorkLogs シートを CSV 保存したものを指定してください。".into());
    }

    // GAS normalizeLogRow_ と同じ別名解決 (旧スキーマ対応)
    let col = |record: &csv::StringRecord, names: &[&str]| -> String {
        for name in names {
            if let Some(&i) = index.get(name) {
                if let Some(value) = record.get(i) {
                    if !value.trim().is_empty() {
                        return value.trim().to_string();
                    }
                }
            }
        }
        String::new()
    };

    let mut imported = 0i64;
    let mut skipped = 0i64;

    let state = app.state::<AppState>();
    for record in reader.records() {
        let record = record.map_err(|e| format!("CSV の解析に失敗しました: {e}"))?;

        let log_id = col(&record, &["logId"]);
        if log_id.is_empty() {
            skipped += 1;
            continue;
        }

        let start_raw = col(&record, &["startTime", "startAt"]);
        let end_raw = col(&record, &["endTime", "endAt"]);
        let start = parse_datetime(&start_raw);
        let end = parse_datetime(&end_raw);

        let mut duration_seconds: i64 = col(&record, &["durationSeconds"]).parse().unwrap_or(0);
        let duration_minutes: i64 = col(&record, &["durationMinutes", "minutes"])
            .parse()
            .unwrap_or(0);
        if duration_seconds <= 0 {
            duration_seconds = match (&start, &end) {
                (Some(s), Some(e)) => ((*e - *s).num_seconds()).max(0),
                _ => duration_minutes * 60,
            };
        }

        let log_date_raw = col(&record, &["logDate", "date"]);
        let log_date = normalize_date_text(&log_date_raw)
            .or_else(|| start.as_ref().map(time::jst_date_text))
            .unwrap_or_default();

        let action = col(&record, &["actionType", "action"]);
        let action_type = if action == "completed" { "completed" } else { "paused" };

        let row = FullLogRow {
            log_id,
            user_id: col(&record, &["userId"]),
            task_list_id: col(&record, &["taskListId"]),
            task_list_name: col(&record, &["taskListName"]),
            task_id: col(&record, &["taskId"]),
            task_title: col(&record, &["taskTitle"]),
            action_type: action_type.to_string(),
            start_time: start.map(|d| time::to_iso(&d)).unwrap_or(start_raw),
            end_time: end.map(|d| time::to_iso(&d)).unwrap_or(end_raw),
            duration_seconds,
            duration_minutes: time::ceil_minutes(duration_seconds),
            log_date,
            memo: col(&record, &["memo"]),
            created_at: col(&record, &["createdAt"]),
            end_reason: {
                let er = col(&record, &["endReason"]);
                if er.is_empty() { "user".to_string() } else { er }
            },
            source: "import_gas".to_string(),
        };

        let conn = state.db.lock().unwrap();
        if repos::insert_imported_log(&conn, &row).map_err(db_err)? {
            imported += 1;
        } else {
            skipped += 1;
        }
    }

    emit_tasks_changed(&app);
    Ok(Some(ImportResult { imported, skipped }))
}

/// ISO / "yyyy-MM-dd HH:mm[:ss]" / "yyyy/MM/dd HH:mm[:ss]" を受け付ける。
/// タイムゾーンなしは JST とみなす (GAS 版の表示形式対応)。
fn parse_datetime(text: &str) -> Option<DateTime<Utc>> {
    if text.is_empty() {
        return None;
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(text) {
        return Some(dt.with_timezone(&Utc));
    }
    for format in [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y/%m/%d %H:%M:%S",
        "%Y/%m/%d %H:%M",
    ] {
        if let Ok(naive) = NaiveDateTime::parse_from_str(text, format) {
            if let Some(jst) = time::TIMEZONE.from_local_datetime(&naive).single() {
                return Some(jst.with_timezone(&Utc));
            }
        }
    }
    None
}

fn normalize_date_text(text: &str) -> Option<String> {
    if text.len() >= 10 && text.as_bytes()[4] == b'-' && text.as_bytes()[7] == b'-' {
        return Some(text[0..10].to_string());
    }
    parse_datetime(text).map(|d| time::jst_date_text(&d))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_iso_datetime() {
        let dt = parse_datetime("2026-05-31T00:00:00.000Z").unwrap();
        assert_eq!(time::to_iso(&dt), "2026-05-31T00:00:00.000Z");
    }

    #[test]
    fn parses_naive_datetime_as_jst() {
        // GAS 版の表示形式 "2026-05-31 09:00" は JST → UTC で 00:00
        let dt = parse_datetime("2026-05-31 09:00").unwrap();
        assert_eq!(time::to_iso(&dt), "2026-05-31T00:00:00.000Z");
        let dt = parse_datetime("2026/05/31 09:00:30").unwrap();
        assert_eq!(time::to_iso(&dt), "2026-05-31T00:00:30.000Z");
    }

    #[test]
    fn normalizes_date_text() {
        assert_eq!(
            normalize_date_text("2026-05-31").as_deref(),
            Some("2026-05-31")
        );
        assert_eq!(
            normalize_date_text("2026-05-31T15:00:00.000Z").as_deref(),
            Some("2026-05-31")
        );
        // JST 深夜 0:30 = UTC 前日 15:30 → logDate は JST 基準
        assert_eq!(
            normalize_date_text("2026/06/01 00:30:00").as_deref(),
            Some("2026-06-01")
        );
        assert_eq!(normalize_date_text(""), None);
    }
}
