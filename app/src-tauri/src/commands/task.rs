use tauri::State;

use super::{db_err, emit_tasks_changed, CmdResult};
use crate::db::repos;
use crate::domain::models::{AppStatus, TaskItem, TaskListOption, TaskRef};
use crate::domain::time;
use crate::state::AppState;

/// 追加先を選ぶための既知タスクリスト一覧 (spec拡張: その場でタスク追加)。
#[tauri::command]
pub fn get_task_lists(state: State<'_, AppState>) -> CmdResult<Vec<TaskListOption>> {
    let conn = state.db.lock().unwrap();
    let lists = repos::fetch_task_lists(&conn).map_err(db_err)?;
    Ok(lists
        .into_iter()
        .map(|(task_list_id, task_list_name)| TaskListOption {
            task_list_id,
            task_list_name,
        })
        .collect())
}

/// タスクをその場で追加する (spec拡張)。ローカル専用タスクとして作成され、
/// 追加直後から「今日やる」リストに表示される。Google Tasks へは同期しない。
#[tauri::command]
pub fn create_task(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    task_list_id: String,
    title: String,
) -> CmdResult<TaskItem> {
    let title = title.trim();
    if title.is_empty() {
        return Err("タスク名を入力してください。".into());
    }

    let item = {
        let conn = state.db.lock().unwrap();
        let task_id = repos::insert_local_task(&conn, &task_list_id, title).map_err(db_err)?;
        let row = repos::get_task(&conn, &task_list_id, &task_id)
            .map_err(db_err)?
            .ok_or("タスクの作成に失敗しました。")?;

        let is_overdue = time::is_overdue(&row.due);
        TaskItem {
            task_list_id: row.task_list_id,
            task_list_name: row.task_list_title,
            task_id: row.id,
            title: row.title,
            notes: row.notes,
            due: row.due,
            status: row.status,
            app_status: AppStatus::NotStarted,
            today_duration_seconds: 0,
            today_duration_minutes: 0,
            is_overdue,
        }
    };

    emit_tasks_changed(&app);
    Ok(item)
}

/// タスクを削除する (spec拡張)。ソフトデリートし、Google Tasks 同期済みタスクは
/// sync_queue 経由でリモートからも削除する。実行中のタスクは削除できない。
#[tauri::command]
pub fn delete_task(app: tauri::AppHandle, state: State<'_, AppState>, task: TaskRef) -> CmdResult<()> {
    {
        let conn = state.db.lock().unwrap();

        if let Some(active) = repos::get_active_session(&conn).map_err(db_err)? {
            if active.task_list_id == task.task_list_id && active.task_id == task.task_id {
                return Err("実行中のタスクは削除できません。先に中断または完了してください。".into());
            }
        }

        repos::get_task(&conn, &task.task_list_id, &task.task_id)
            .map_err(db_err)?
            .ok_or("タスクが見つかりません。")?;

        let tx = conn.unchecked_transaction().map_err(db_err)?;
        repos::mark_task_deleted_locally(&tx, &task.task_list_id, &task.task_id).map_err(db_err)?;
        repos::enqueue_sync_op(
            &tx,
            "delete_task",
            &task.task_list_id,
            &task.task_id,
            "{}",
        )
        .map_err(db_err)?;
        tx.commit().map_err(db_err)?;
    }

    emit_tasks_changed(&app);
    crate::google::kick_sync(&app);
    Ok(())
}
