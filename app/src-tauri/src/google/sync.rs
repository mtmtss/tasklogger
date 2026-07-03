//! Pull / Push 同期 (spec §6.2, §6.3)。すべて blocking で、専用スレッドから呼ばれる。

use rusqlite::params;
use tauri::{Emitter, Manager};

use super::tasks_api::{self, ApiError};
use super::GoogleState;
use crate::db::repos;
use crate::domain::time;
use crate::state::AppState;

struct QueueRow {
    id: i64,
    task_list_id: String,
    task_id: String,
    payload: serde_json::Value,
}

/// push → pull を 1 サイクル実行する。未接続なら何もしない。
pub fn perform_sync(app: &tauri::AppHandle) -> Result<(), String> {
    let google = app.state::<GoogleState>();

    // 同時実行ガード
    let _guard = match google.sync_lock.try_lock() {
        Ok(g) => g,
        Err(_) => return Ok(()), // 別の同期が進行中
    };

    let token = match super::get_access_token(app)? {
        Some(token) => token,
        None => return Ok(()), // 未接続 (オフラインモード扱い)
    };

    let push_result = push_queue(app, &token);
    let pull_result = pull_tasks(app, &token);

    let _ = app.emit("tasks-changed", ());
    let _ = app.emit("sync-status-changed", ());

    push_result?;
    pull_result
}

fn push_queue(app: &tauri::AppHandle, token: &str) -> Result<(), String> {
    let state = app.state::<AppState>();

    let rows: Vec<QueueRow> = {
        let conn = state.db.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT id, task_list_id, task_id, payload FROM sync_queue ORDER BY id")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| e.to_string())?;
        rows.into_iter()
            .map(|(id, list, task, payload)| QueueRow {
                id,
                task_list_id: list,
                task_id: task,
                payload: serde_json::from_str(&payload).unwrap_or(serde_json::json!({})),
            })
            .collect()
    };

    for row in rows {
        match tasks_api::patch_task(token, &row.task_list_id, &row.task_id, &row.payload) {
            Ok(()) => {
                let conn = state.db.lock().unwrap();
                let _ = conn.execute("DELETE FROM sync_queue WHERE id = ?1", params![row.id]);
                let _ = conn.execute(
                    "UPDATE tasks SET dirty = 0 WHERE task_list_id = ?1 AND id = ?2",
                    params![row.task_list_id, row.task_id],
                );
            }
            Err(ApiError::Gone) => {
                // リモートで削除済み → 破棄して通知 (spec §6.3)
                let conn = state.db.lock().unwrap();
                let _ = conn.execute("DELETE FROM sync_queue WHERE id = ?1", params![row.id]);
                let _ = conn.execute(
                    "UPDATE tasks SET dirty = 0, deleted = 1 WHERE task_list_id = ?1 AND id = ?2",
                    params![row.task_list_id, row.task_id],
                );
                let _ = app.emit(
                    "sync-error",
                    "同期しようとしたタスクは Google 側で削除されていました。",
                );
            }
            Err(err) => {
                // ネットワーク/一時エラー → attempts を上げて次サイクルで再試行
                let message = err.to_string();
                let conn = state.db.lock().unwrap();
                let _ = conn.execute(
                    "UPDATE sync_queue SET attempts = attempts + 1, last_error = ?2 WHERE id = ?1",
                    params![row.id, message],
                );
                return Err(message);
            }
        }
    }
    Ok(())
}

fn pull_tasks(app: &tauri::AppHandle, token: &str) -> Result<(), String> {
    let state = app.state::<AppState>();
    let pull_started = time::to_iso(&time::now_utc());

    let lists = tasks_api::list_tasklists(token).map_err(|e| e.to_string())?;

    // リスト upsert
    {
        let conn = state.db.lock().unwrap();
        for list in &lists {
            conn.execute(
                "INSERT INTO task_lists (id, title, updated, deleted, fetched_at)
                 VALUES (?1, ?2, ?3, 0, ?4)
                 ON CONFLICT(id) DO UPDATE SET
                   title = excluded.title, updated = excluded.updated,
                   deleted = 0, fetched_at = excluded.fetched_at",
                params![list.id, list.title, list.updated, pull_started],
            )
            .map_err(|e| e.to_string())?;
        }
        // 今回見えなかったリストは削除扱い (サンプルデータ等ローカル専用 ID は除外)
        conn.execute(
            "UPDATE task_lists SET deleted = 1
             WHERE fetched_at < ?1 AND id NOT LIKE 'sample-%'",
            params![pull_started],
        )
        .map_err(|e| e.to_string())?;
    }

    for list in &lists {
        let tasks = tasks_api::list_open_tasks(token, &list.id).map_err(|e| e.to_string())?;
        let conn = state.db.lock().unwrap();
        for task in tasks {
            // dirty=1 (未 push のローカル変更) は上書きしない (spec §6.2)
            conn.execute(
                "INSERT INTO tasks
                   (id, task_list_id, title, notes, due, status, position, updated, deleted, dirty, fetched_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, 0, ?9)
                 ON CONFLICT(id) DO UPDATE SET
                   task_list_id = excluded.task_list_id,
                   title = excluded.title, notes = excluded.notes,
                   due = excluded.due, status = excluded.status,
                   position = excluded.position, updated = excluded.updated,
                   deleted = 0, fetched_at = excluded.fetched_at
                 WHERE tasks.dirty = 0",
                params![
                    task.id,
                    list.id,
                    task.title.unwrap_or_default(),
                    task.notes,
                    task.due,
                    task.status,
                    task.position,
                    task.updated,
                    pull_started
                ],
            )
            .map_err(|e| e.to_string())?;
        }
        // 未完了フェッチに現れなかった行 = リモートで完了/削除された未完了タスク。
        // ローカル未 push 変更 (dirty) は保護する
        conn.execute(
            "UPDATE tasks SET deleted = 1
             WHERE task_list_id = ?1 AND dirty = 0 AND fetched_at < ?2
               AND id NOT LIKE 'sample-%'",
            params![list.id, pull_started],
        )
        .map_err(|e| e.to_string())?;
    }

    {
        let conn = state.db.lock().unwrap();
        repos::set_setting(&conn, "last_pull_at", &pull_started).map_err(|e| e.to_string())?;
    }
    Ok(())
}
