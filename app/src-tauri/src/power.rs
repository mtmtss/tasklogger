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
const HEARTBEAT_INTERVAL_SECS: u64 = 15;
const GAP_THRESHOLD_SECS: i64 = 90;
/// 無操作自動中断のデフォルトしきい値 (分)。設定 idle_pause_minutes で変更、0 で無効。
const DEFAULT_IDLE_PAUSE_MINUTES: i64 = 5;

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

        // 自動中断はユーザーが見ていないときに起きるため OS 通知も出す (spec §11 M5)
        if set_interrupted {
            use tauri_plugin_notification::NotificationExt;
            let reason_text = match end_reason {
                "idle" => "無操作のため",
                "sleep" => "スリープのため",
                "recovery" => "前回終了時に",
                _ => "",
            };
            let _ = app
                .notification()
                .builder()
                .title("TaskLogger: タスクを自動中断しました")
                .body(format!("{reason_text}作業タイマーを停止しました。"))
                .show();
        }
    }
    paused
}

/// スリープ突入 (spec §7.2)。
pub fn on_suspend(app: &tauri::AppHandle) {
    auto_pause(app, None, "sleep", true);
}

/// 復帰 (spec §7.2): pull 同期を促し、中断タスクがあれば再開ダイアログを出させる。
///
/// 競合対策: PBT_APMSUSPEND の処理スレッドが suspend 前に走り切れなかった場合、
/// 復帰時点でセッションが残っている。その場合はここでハートビート時刻で締める
/// (スリープ時間を作業時間に含めない)。
pub fn on_resume(app: &tauri::AppHandle) {
    let stale_heartbeat: Option<String> = {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap();
        match repos::get_session_heartbeat(&conn) {
            Ok(Some(heartbeat)) => {
                let gap = time::parse_iso(&heartbeat)
                    .map(|hb| (time::now_utc() - hb).num_seconds())
                    .unwrap_or(0);
                (gap > GAP_THRESHOLD_SECS).then_some(heartbeat)
            }
            _ => None,
        }
    };
    if let Some(end_iso) = stale_heartbeat {
        auto_pause(app, Some(end_iso), "sleep", true);
    }

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

/// 無操作しきい値 (秒)。設定 idle_pause_minutes (デフォルト 5 分)。0 以下で無効。
fn idle_threshold_secs(app: &tauri::AppHandle) -> Option<i64> {
    let state = app.state::<AppState>();
    let conn = state.db.lock().unwrap();
    let minutes = repos::get_setting(&conn, "idle_pause_minutes")
        .ok()
        .flatten()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .unwrap_or(DEFAULT_IDLE_PAUSE_MINUTES);
    (minutes > 0).then_some(minutes * 60)
}

/// ユーザーが戻ってきたとき: pull 同期 + 未応答の中断タスクがあれば再開ダイアログ。
fn on_user_returned(app: &tauri::AppHandle) {
    crate::google::kick_sync(app);
    if let Some(task) = load_interrupted(app) {
        let _ = app.emit("power-resumed", task);
    }
}

/// ハートビートループ (spec §7.1)。15 秒毎に以下を行う:
/// 1. ユーザー復帰検知 (無操作 → 入力再開) → 再開ダイアログ
/// 2. 検知漏れスリープ (前回ハートビートとの壁時計ギャップ > 90 秒) → sleep 中断
/// 3. 無操作しきい値超過 (ロック/スクリーンセーバ/離席) → idle 中断
///    終了時刻は「最後に入力があった時刻」で締める
/// 4. running セッションのハートビート更新
pub fn spawn_heartbeat(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let mut prev_idle: i64 = 0;
        loop {
            std::thread::sleep(std::time::Duration::from_secs(HEARTBEAT_INTERVAL_SECS));

            let idle = crate::platform::idle_seconds().unwrap_or(0);
            let threshold = idle_threshold_secs(&app);

            // 1. 復帰検知: しきい値を超える無操作から入力が再開した
            if let Some(th) = threshold {
                if prev_idle >= th && idle < prev_idle {
                    on_user_returned(&app);
                }
            }
            prev_idle = idle;

            // セッションが無ければ以降は何もしない
            let heartbeat: Option<String> = {
                let state = app.state::<AppState>();
                let conn = state.db.lock().unwrap();
                repos::get_session_heartbeat(&conn).ok().flatten()
            };
            let Some(heartbeat) = heartbeat else { continue };

            // 2. 検知漏れスリープ
            let gap = time::parse_iso(&heartbeat)
                .map(|hb| (time::now_utc() - hb).num_seconds())
                .unwrap_or(0);
            if gap > GAP_THRESHOLD_SECS {
                auto_pause(&app, Some(heartbeat), "sleep", true);
                on_user_returned(&app);
                continue;
            }

            // 3. 無操作自動中断 (spec §7.1): end_time = 最終入力時刻
            if let Some(th) = threshold {
                if idle >= th {
                    let last_input = time::now_utc() - chrono::Duration::seconds(idle);
                    auto_pause(&app, Some(time::to_iso(&last_input)), "idle", true);
                    continue;
                }
            }

            // 4. ハートビート更新
            let state = app.state::<AppState>();
            let conn = state.db.lock().unwrap();
            let _ = conn.execute(
                "UPDATE active_session SET last_heartbeat_at = ?1 WHERE id = 1",
                rusqlite::params![time::to_iso(&time::now_utc())],
            );
        }
    });
}
