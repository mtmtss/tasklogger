//! スリープ/復帰/異常終了時のセッション処理 (spec §7)。
//! OS 依存の検知は platform/ に隔離し、ここは共通処理のみ。

use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};

use crate::db::repos;
use crate::domain::models::ActiveSessionView;
use crate::domain::time;
use crate::state::AppState;

pub const INTERRUPTED_KEY: &str = "sleep_interrupted_task";

/// ハートビート間隔と、検知漏れスリープとみなす壁時計ギャップ (spec §7.1)。
const HEARTBEAT_INTERVAL_SECS: u64 = 30;
const GAP_THRESHOLD_SECS: i64 = 90;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterruptedTask {
    pub task_list_id: String,
    pub task_id: String,
    pub task_title: String,
}

/// running セッションを中断ログにして閉じる。
/// `end_time_iso` が None なら現在時刻。`set_interrupted` で復帰ダイアログ対象に記録する。
pub fn auto_pause(
    app: &tauri::AppHandle,
    end_time_iso: Option<String>,
    end_reason: &str,
    set_interrupted: bool,
) -> bool {
    let state = app.state::<AppState>();
    let paused = {
        let conn = state.db.lock().unwrap();
        let session = match repos::get_active_session(&conn) {
            Ok(Some(s)) => s,
            _ => return false,
        };

        let end_iso = end_time_iso.unwrap_or_else(|| time::to_iso(&time::now_utc()));
        let duration = match (time::parse_iso(&session.start_at), time::parse_iso(&end_iso)) {
            (Some(start), Some(end)) => (end - start).num_seconds().max(0),
            _ => 0,
        };

        let _ = repos::append_work_log(
            &conn,
            &repos::NewWorkLog {
                task_list_id: &session.task_list_id,
                task_list_name: &session.task_list_name,
                task_id: &session.task_id,
                task_title: &session.task_title,
                action_type: "paused",
                start_time: session.start_at.clone(),
                end_time: end_iso,
                duration_seconds: duration,
                memo: "",
                end_reason,
            },
        );
        let _ = repos::clear_active_session(&conn);

        if set_interrupted {
            let task = InterruptedTask {
                task_list_id: session.task_list_id,
                task_id: session.task_id,
                task_title: session.task_title,
            };
            if let Ok(json) = serde_json::to_string(&task) {
                let _ = repos::set_setting(&conn, INTERRUPTED_KEY, &json);
            }
        }
        true
    };

    if paused {
        let _ = app.emit("session-changed", None::<ActiveSessionView>);
        let _ = app.emit("tasks-changed", ());
    }
    paused
}

/// スリープ突入 (spec §7.2)。
pub fn on_suspend(app: &tauri::AppHandle) {
    auto_pause(app, None, "sleep", true);
}

/// 復帰 (spec §7.2): pull 同期を促し、中断タスクがあれば再開ダイアログを出させる。
pub fn on_resume(app: &tauri::AppHandle) {
    crate::google::kick_sync(app);
    if let Some(task) = load_interrupted(app) {
        let _ = app.emit("power-resumed", task);
    }
}

pub fn load_interrupted(app: &tauri::AppHandle) -> Option<InterruptedTask> {
    let state = app.state::<AppState>();
    let conn = state.db.lock().unwrap();
    let json = repos::get_setting(&conn, INTERRUPTED_KEY).ok()??;
    serde_json::from_str(&json).ok()
}

pub fn clear_interrupted(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    let conn = state.db.lock().unwrap();
    let _ = repos::delete_setting(&conn, INTERRUPTED_KEY);
}

/// ハートビート + 検知漏れスリープの保険 (spec §7.1 二次検知)。
/// PBT 通知を取りこぼした場合でも、前回ハートビートとの壁時計差で検出する。
pub fn spawn_heartbeat(app: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(HEARTBEAT_INTERVAL_SECS));

        let gap_end: Option<String> = {
            let state = app.state::<AppState>();
            let conn = state.db.lock().unwrap();
            match repos::get_session_heartbeat(&conn) {
                Ok(Some(heartbeat)) => {
                    let gap = time::parse_iso(&heartbeat)
                        .map(|hb| (time::now_utc() - hb).num_seconds())
                        .unwrap_or(0);
                    if gap > GAP_THRESHOLD_SECS {
                        Some(heartbeat) // 検知漏れ: 実際に動いていた最後の時刻で締める
                    } else {
                        let _ = conn.execute(
                            "UPDATE active_session SET last_heartbeat_at = ?1 WHERE id = 1",
                            rusqlite::params![time::to_iso(&time::now_utc())],
                        );
                        None
                    }
                }
                _ => None,
            }
        };

        if let Some(end_iso) = gap_end {
            auto_pause(&app, Some(end_iso), "sleep", true);
            on_resume(&app);
        }
    });
}
