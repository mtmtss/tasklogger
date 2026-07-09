use std::collections::HashMap;

use tauri::State;

use super::{db_err, emit_tasks_changed, CmdResult};
use crate::db::repos;
use crate::state::AppState;

/// UI で扱う設定キーのホワイトリスト。秘密情報 (トークン) は keyring のみで扱う。
const ALLOWED_KEYS: &[&str] = &[
    "float_window_position",
    "autostart",
    "close_to_tray",
    "idle_pause_minutes",
    "user_email",
    "oauth_client_id",
    "oauth_client_secret",
    "sheet_sync_enabled",
    "log_spreadsheet_id",
    "last_sheet_sync_at",
    "ai_model",
    "ai_user_context",
    "ai_auto_plan",
    "capture_hotkey",
    "capture_hotkey_error",
];

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> CmdResult<HashMap<String, String>> {
    let conn = state.db.lock().unwrap();
    let mut map = HashMap::new();
    for key in ALLOWED_KEYS {
        if let Some(value) = repos::get_setting(&conn, key).map_err(db_err)? {
            map.insert(key.to_string(), value);
        }
    }
    Ok(map)
}

#[tauri::command]
pub fn set_setting(state: State<'_, AppState>, key: String, value: String) -> CmdResult<()> {
    if !ALLOWED_KEYS.contains(&key.as_str()) {
        return Err(format!("不明な設定キー: {key}"));
    }
    let conn = state.db.lock().unwrap();
    repos::set_setting(&conn, &key, &value).map_err(db_err)
}

/// OS ログイン時の自動起動を切り替える (spec §11 M5)。設定にも永続化する。
#[tauri::command]
pub fn set_autostart(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> CmdResult<()> {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    let result = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    result.map_err(|e| format!("自動起動の設定に失敗しました: {e}"))?;

    let conn = state.db.lock().unwrap();
    repos::set_setting(&conn, "autostart", if enabled { "true" } else { "false" })
        .map_err(db_err)
}

/// 開発用: サンプルタスクを投入してオフラインで UI を確認できるようにする。
#[tauri::command]
pub fn seed_sample_data(app: tauri::AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    {
        let conn = state.db.lock().unwrap();
        repos::seed_sample_data(&conn).map_err(db_err)?;
    }
    emit_tasks_changed(&app);
    Ok(())
}
