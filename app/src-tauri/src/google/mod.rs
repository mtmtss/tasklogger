//! Google Tasks 連携 (spec §6)。
//! ローカルファースト: UI 操作は SQLite で完結し、ここは裏で pull/push するだけ。

pub mod auth;
pub mod sync;
pub mod tasks_api;

use std::sync::{mpsc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{Emitter, Manager, State};

use crate::commands::{db_err, CmdResult};
use crate::db::repos;
use crate::state::AppState;

/// 同期周期 (spec §6.2: 5 分)。
const SYNC_INTERVAL: Duration = Duration::from_secs(300);

pub struct GoogleState {
    /// access token (メモリのみ, spec §6.1)
    pub tokens: Mutex<Option<auth::TokenSet>>,
    /// 同期の同時実行ガード
    pub sync_lock: Mutex<()>,
    /// ワーカーを即時に起こすためのチャネル
    kick_tx: mpsc::Sender<()>,
}

/// setup で呼ぶ: GoogleState を登録し、同期ワーカースレッドを起動する。
pub fn init(app: &tauri::AppHandle) {
    let (tx, rx) = mpsc::channel::<()>();
    app.manage(GoogleState {
        tokens: Mutex::new(None),
        sync_lock: Mutex::new(()),
        kick_tx: tx,
    });

    let handle = app.clone();
    std::thread::spawn(move || {
        // 起動直後に 1 回 (未接続なら即 return する)
        let _ = sync::perform_sync(&handle);
        loop {
            // kick が来たら即時、来なければ 5 分周期
            let _ = rx.recv_timeout(SYNC_INTERVAL);
            if let Err(err) = sync::perform_sync(&handle) {
                let _ = handle.emit("sync-error", err);
            }
        }
    });
}

/// 変更操作の直後に同期を促す (ノンブロッキング)。
pub fn kick_sync(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<GoogleState>() {
        let _ = state.kick_tx.send(());
    }
}

/// 有効な access token を返す。未接続なら Ok(None)。必要なら refresh する。
pub fn get_access_token(app: &tauri::AppHandle) -> Result<Option<String>, String> {
    let google = app.state::<GoogleState>();

    {
        let tokens = google.tokens.lock().unwrap();
        if let Some(t) = tokens.as_ref() {
            if t.expires_at - 60 > chrono::Utc::now().timestamp() {
                return Ok(Some(t.access_token.clone()));
            }
        }
    }

    let refresh_token = match auth::load_refresh_token() {
        Some(t) => t,
        None => return Ok(None),
    };

    let (client_id, client_secret) = load_client_config(app)?
        .ok_or("OAuth クライアント ID が未設定です。設定ページで入力してください。")?;

    let token_set = auth::refresh_access_token(&client_id, &client_secret, &refresh_token)?;
    let access_token = token_set.access_token.clone();
    *google.tokens.lock().unwrap() = Some(token_set);
    Ok(Some(access_token))
}

fn load_client_config(app: &tauri::AppHandle) -> Result<Option<(String, String)>, String> {
    let state = app.state::<AppState>();
    let conn = state.db.lock().unwrap();
    let client_id = repos::get_setting(&conn, "oauth_client_id").map_err(db_err)?;
    let client_secret = repos::get_setting(&conn, "oauth_client_secret")
        .map_err(db_err)?
        .unwrap_or_default();
    Ok(client_id
        .filter(|id| !id.is_empty())
        .map(|id| (id, client_secret)))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub connected: bool,
    pub last_pull_at: Option<String>,
    pub queue_count: i64,
}

/// Google と接続する (OAuth 認可フロー)。ブラウザが開き、完了までブロッキングになるため
/// async コマンド + spawn_blocking で UI を止めない。
#[tauri::command]
pub async fn connect_google(
    app: tauri::AppHandle,
    client_id: String,
    client_secret: String,
) -> CmdResult<()> {
    if client_id.trim().is_empty() {
        return Err("クライアント ID を入力してください。".into());
    }

    // 入力値を設定に保存してからフロー実行
    {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap();
        repos::set_setting(&conn, "oauth_client_id", client_id.trim()).map_err(db_err)?;
        repos::set_setting(&conn, "oauth_client_secret", client_secret.trim()).map_err(db_err)?;
    }

    let id = client_id.trim().to_string();
    let secret = client_secret.trim().to_string();
    let handle = app.clone();
    let (token_set, refresh_token) = tauri::async_runtime::spawn_blocking(move || {
        auth::run_authorization_flow(&handle, &id, &secret)
    })
    .await
    .map_err(|e| format!("認証処理に失敗しました: {e}"))??;

    auth::save_refresh_token(&refresh_token)?;
    {
        let google = app.state::<GoogleState>();
        *google.tokens.lock().unwrap() = Some(token_set);
    }

    let _ = app.emit("sync-status-changed", ());
    kick_sync(&app);
    Ok(())
}

/// 接続解除: keyring の refresh token とメモリ上のトークンを破棄する。
#[tauri::command]
pub fn disconnect_google(app: tauri::AppHandle) -> CmdResult<()> {
    auth::delete_refresh_token();
    {
        let google = app.state::<GoogleState>();
        *google.tokens.lock().unwrap() = None;
    }
    let _ = app.emit("sync-status-changed", ());
    Ok(())
}

/// 手動同期。完了までの結果を返す (エラーはメッセージとして表示可能)。
#[tauri::command]
pub async fn sync_now(app: tauri::AppHandle) -> CmdResult<()> {
    let connected = auth::load_refresh_token().is_some();
    if !connected {
        return Err("Google と未接続です。設定ページから接続してください。".into());
    }
    tauri::async_runtime::spawn_blocking(move || sync::perform_sync(&app))
        .await
        .map_err(|e| format!("同期処理に失敗しました: {e}"))?
}

#[tauri::command]
pub fn get_sync_status(app: tauri::AppHandle, state: State<'_, AppState>) -> CmdResult<SyncStatus> {
    let conn = state.db.lock().unwrap();
    let _ = &app;
    Ok(SyncStatus {
        connected: auth::load_refresh_token().is_some(),
        last_pull_at: repos::get_setting(&conn, "last_pull_at").map_err(db_err)?,
        queue_count: repos::sync_queue_count(&conn).map_err(db_err)?,
    })
}
