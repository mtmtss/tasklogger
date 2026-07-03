pub mod dashboard;
pub mod session;
pub mod settings;

use tauri::Emitter;

use crate::db::repos;
use crate::domain::models::ActiveSessionView;
use crate::domain::{status, time};
use crate::state::AppState;

pub type CmdResult<T> = Result<T, String>;

pub fn db_err(e: rusqlite::Error) -> String {
    format!("データベースエラー: {e}")
}

/// 現在のアクティブセッションをビューに変換。
pub fn build_session_view(conn: &rusqlite::Connection) -> CmdResult<Option<ActiveSessionView>> {
    let session = repos::get_active_session(conn).map_err(db_err)?;
    Ok(session.map(|s| {
        let elapsed = status::elapsed_seconds(&s.start_at, time::now_utc().timestamp_millis());
        ActiveSessionView {
            task_list_id: s.task_list_id,
            task_list_name: s.task_list_name,
            task_id: s.task_id,
            task_title: s.task_title,
            start_at: s.start_at,
            elapsed_seconds: elapsed,
        }
    }))
}

/// セッション変更を全ウィンドウへ通知 (spec §6.5, §8.3)。
pub fn emit_session_changed(app: &tauri::AppHandle, state: &AppState) {
    let payload = {
        let conn = state.db.lock().unwrap();
        build_session_view(&conn).ok().flatten()
    };
    let _ = app.emit("session-changed", payload);
}

pub fn emit_tasks_changed(app: &tauri::AppHandle) {
    let _ = app.emit("tasks-changed", ());
}
