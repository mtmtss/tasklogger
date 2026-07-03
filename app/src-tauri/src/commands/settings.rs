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
    "user_email",
    "oauth_client_id",
    "oauth_client_secret",
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
