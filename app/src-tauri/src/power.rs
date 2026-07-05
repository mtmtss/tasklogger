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
    {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap();
        let _ = close_null_tracking(&conn, &time::to_iso(&time::now_utc()), "sleep");
    }
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

// ---- null タスク記録 (spec §5.7) ----------------------------------------
// タスク未選択時の PC 操作時間を「null」という擬似タスクとして記録する。
// 目的: タスク開始を忘れても作業時間が失われない (記録ミスの影響最小化)。
// active_session は使わず (状態機械の不変条件を守る)、settings に開始時刻を持つ。

const NULL_TASK: &str = "null";
/// これ未満の細切れは記録しない (ノイズ防止)
const NULL_MIN_LOG_SECS: i64 = 60;
/// 開始判定: 無操作がこの秒数以内なら「操作中」とみなす
const NULL_START_MAX_IDLE_SECS: i64 = 60;

const NULL_STARTED_KEY: &str = "null_session_started_at";
const NULL_LAST_SEEN_KEY: &str = "null_session_last_seen";

fn null_tracking_enabled(conn: &rusqlite::Connection) -> bool {
    // 既定で有効。"false" のときのみ無効
    !matches!(
        repos::get_setting(conn, "null_tracking_enabled"),
        Ok(Some(v)) if v == "false"
    )
}

/// 開始時刻の候補を、既存ログの最終 end_time より前に遡らないようクランプする
/// (直前まで実タスクを実行していた時間との二重計上を防ぐ)。
pub(crate) fn clamp_null_start(candidate_iso: String, latest_end: Option<String>) -> String {
    match latest_end {
        Some(end) if end > candidate_iso => end,
        _ => candidate_iso,
    }
}

/// null セッションを閉じてログを書く。開始していなければ何もしない。
/// 60 秒未満は破棄。戻り値 = ログを書いたか。
pub fn close_null_tracking(
    conn: &rusqlite::Connection,
    end_iso: &str,
    end_reason: &str,
) -> bool {
    let started = match repos::get_setting(conn, NULL_STARTED_KEY) {
        Ok(Some(v)) if !v.is_empty() => v,
        _ => return false,
    };
    let _ = repos::delete_setting(conn, NULL_STARTED_KEY);
    let _ = repos::delete_setting(conn, NULL_LAST_SEEN_KEY);

    let duration = match (time::parse_iso(&started), time::parse_iso(end_iso)) {
        (Some(s), Some(e)) => (e - s).num_seconds(),
        _ => return false,
    };
    if duration < NULL_MIN_LOG_SECS {
        return false;
    }

    let _ = repos::append_work_log(
        conn,
        &repos::NewWorkLog {
            task_list_id: NULL_TASK,
            task_list_name: NULL_TASK,
            task_id: NULL_TASK,
            task_title: NULL_TASK,
            action_type: "paused",
            start_time: started,
            end_time: end_iso.to_string(),
            duration_seconds: duration,
            memo: "",
            end_reason,
        },
    );
    true
}

/// 実タスク開始時に呼ぶ: 直前までの null 時間をその場で締める (spec §5.7)。
pub fn close_null_for_task_start(conn: &rusqlite::Connection) -> bool {
    close_null_tracking(conn, &time::to_iso(&time::now_utc()), "user")
}

/// ハートビート毎の null 追跡。実セッションの有無と無操作時間で開始/更新/終了する。
/// ログを書いたら true (呼び出し側で tasks-changed を emit する)。
fn null_tick(
    conn: &rusqlite::Connection,
    idle: i64,
    threshold: Option<i64>,
    has_real_session: bool,
) -> bool {
    if !null_tracking_enabled(conn) {
        // 無効化されたら開いている区間は最後に見た時刻で締める
        if let Ok(Some(last_seen)) = repos::get_setting(conn, NULL_LAST_SEEN_KEY) {
            return close_null_tracking(conn, &last_seen, "user");
        }
        return false;
    }

    if has_real_session {
        // 通常は start_task 側で締まっている。取りこぼしの保険
        return close_null_tracking(conn, &time::to_iso(&time::now_utc()), "user");
    }

    let now = time::now_utc();
    let started = matches!(
        repos::get_setting(conn, NULL_STARTED_KEY),
        Ok(Some(ref v)) if !v.is_empty()
    );

    if !started {
        // ユーザーが操作中なら記録開始 (開始点 = 最後に入力があった時刻)
        if idle <= NULL_START_MAX_IDLE_SECS {
            let candidate = time::to_iso(&(now - chrono::Duration::seconds(idle)));
            let latest_end = repos::latest_log_end(conn).ok().flatten();
            let start = clamp_null_start(candidate, latest_end);
            let _ = repos::set_setting(conn, NULL_STARTED_KEY, &start);
            let _ = repos::set_setting(conn, NULL_LAST_SEEN_KEY, &time::to_iso(&now));
        }
        return false;
    }

    // プロセス停止 (スリープ等) を挟んだ場合: 最後に見た時刻で締める
    if let Ok(Some(last_seen)) = repos::get_setting(conn, NULL_LAST_SEEN_KEY) {
        let gap = time::parse_iso(&last_seen)
            .map(|ls| (now - ls).num_seconds())
            .unwrap_or(0);
        if gap > GAP_THRESHOLD_SECS {
            return close_null_tracking(conn, &last_seen, "sleep");
        }
    }

    // 無操作しきい値超過: 最後に入力があった時刻で締める
    let close_threshold = threshold.unwrap_or(DEFAULT_IDLE_PAUSE_MINUTES * 60);
    if idle >= close_threshold {
        let last_input = time::to_iso(&(now - chrono::Duration::seconds(idle)));
        return close_null_tracking(conn, &last_input, "idle");
    }

    let _ = repos::set_setting(conn, NULL_LAST_SEEN_KEY, &time::to_iso(&now));
    false
}

/// 起動時回復: 前回終了時に開いたままの null 区間を最後に見た時刻で締める。
pub fn recover_null_tracking(conn: &rusqlite::Connection) {
    if let Ok(Some(last_seen)) = repos::get_setting(conn, NULL_LAST_SEEN_KEY) {
        let _ = close_null_tracking(conn, &last_seen, "recovery");
    } else {
        let _ = repos::delete_setting(conn, NULL_STARTED_KEY);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repos;
    use crate::domain::time;

    #[test]
    fn clamp_prevents_overlap_with_previous_log() {
        // 直前の実タスクの end_time より前には遡らない
        let clamped = clamp_null_start(
            "2026-07-06T01:00:00.000Z".into(),
            Some("2026-07-06T01:02:00.000Z".into()),
        );
        assert_eq!(clamped, "2026-07-06T01:02:00.000Z");

        // 既存ログの方が古ければ候補をそのまま使う
        let kept = clamp_null_start(
            "2026-07-06T01:00:00.000Z".into(),
            Some("2026-07-06T00:50:00.000Z".into()),
        );
        assert_eq!(kept, "2026-07-06T01:00:00.000Z");

        let no_logs = clamp_null_start("2026-07-06T01:00:00.000Z".into(), None);
        assert_eq!(no_logs, "2026-07-06T01:00:00.000Z");
    }

    #[test]
    fn null_session_below_minimum_is_discarded() {
        let conn = crate::db::open_in_memory().unwrap();
        let start = time::now_utc() - chrono::Duration::seconds(30);
        repos::set_setting(&conn, NULL_STARTED_KEY, &time::to_iso(&start)).unwrap();

        let wrote = close_null_tracking(&conn, &time::to_iso(&time::now_utc()), "user");
        assert!(!wrote, "60 秒未満は記録しない");
        assert!(repos::get_setting(&conn, NULL_STARTED_KEY).unwrap().is_none());
    }

    #[test]
    fn null_session_is_logged_as_null_task() {
        let conn = crate::db::open_in_memory().unwrap();
        let start = time::now_utc() - chrono::Duration::seconds(300);
        repos::set_setting(&conn, NULL_STARTED_KEY, &time::to_iso(&start)).unwrap();

        let wrote = close_null_tracking(&conn, &time::to_iso(&time::now_utc()), "idle");
        assert!(wrote);

        let logs = repos::fetch_logs_by_date(&conn, &time::today_jst()).unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].task_id, "null");
        assert_eq!(logs[0].task_list_id, "null");
        assert_eq!(logs[0].action_type, "paused");
        assert!(logs[0].duration_seconds >= 299);
    }
}

// --------------------------------------------------------------------------

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

            // null タスク追跡 (spec §5.7) + 実セッションのハートビート取得
            let (heartbeat, null_logged) = {
                let state = app.state::<AppState>();
                let conn = state.db.lock().unwrap();
                let hb = repos::get_session_heartbeat(&conn).ok().flatten();
                let logged = null_tick(&conn, idle, threshold, hb.is_some());
                (hb, logged)
            };
            if null_logged {
                let _ = app.emit("tasks-changed", ());
            }

            // セッションが無ければ以降は何もしない
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
