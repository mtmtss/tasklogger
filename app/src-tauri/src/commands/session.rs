use tauri::State;

use super::{db_err, emit_session_changed, emit_tasks_changed, CmdResult};
use crate::db::repos;
use crate::domain::models::TaskRef;
use crate::domain::time;
use crate::state::AppState;

const ERR_ALREADY_RUNNING: &str =
    "現在作業中のタスクがあります。先に中断または完了してください。";

/// 開始 / 再開 (spec §4.2 #1, #4)。別タスク running 中はブロック。
#[tauri::command]
pub fn start_task(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    task: TaskRef,
) -> CmdResult<()> {
    {
        let conn = state.db.lock().unwrap();

        if let Some(active) = repos::get_active_session(&conn).map_err(db_err)? {
            if active.task_list_id == task.task_list_id && active.task_id == task.task_id {
                return Ok(()); // 同一タスクの二重開始は無害 (GAS 版踏襲)
            }
            return Err(ERR_ALREADY_RUNNING.into());
        }

        let row = repos::get_task(&conn, &task.task_list_id, &task.task_id)
            .map_err(db_err)?
            .ok_or("タスクが見つかりません。")?;

        repos::create_active_session(
            &conn,
            &row.task_list_id,
            &row.task_list_title,
            &row.id,
            &row.title,
        )
        .map_err(db_err)?;
    }
    emit_session_changed(&app, &state);
    Ok(())
}

/// 中断 / 完了 (spec §4.2 #2, #3)。実行中セッションを終了してログを書く。
#[tauri::command]
pub fn stop_task(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    action: String,
    memo: Option<String>,
) -> CmdResult<()> {
    if action != "paused" && action != "completed" {
        return Err(format!("不正な action: {action}"));
    }

    {
        let conn = state.db.lock().unwrap();
        let session = repos::get_active_session(&conn)
            .map_err(db_err)?
            .ok_or("作業中のタスクがありません。")?;

        let end = time::now_utc();
        let start = time::parse_iso(&session.start_at)
            .ok_or("開始時刻が見つからないため、作業時間を記録できません。")?;
        let duration_seconds = ((end - start).num_seconds()).max(0);

        let tx = conn.unchecked_transaction().map_err(db_err)?;
        repos::append_work_log(
            &tx,
            &repos::NewWorkLog {
                task_list_id: &session.task_list_id,
                task_list_name: &session.task_list_name,
                task_id: &session.task_id,
                task_title: &session.task_title,
                action_type: &action,
                start_time: session.start_at.clone(),
                end_time: time::to_iso(&end),
                duration_seconds,
                memo: memo.as_deref().unwrap_or(""),
                end_reason: "user",
            },
        )
        .map_err(db_err)?;

        if action == "completed" {
            repos::mark_task_completed_locally(&tx, &session.task_list_id, &session.task_id)
                .map_err(db_err)?;
            repos::enqueue_sync_op(
                &tx,
                "complete_task",
                &session.task_list_id,
                &session.task_id,
                "{}",
            )
            .map_err(db_err)?;
        }

        repos::clear_active_session(&tx).map_err(db_err)?;
        tx.commit().map_err(db_err)?;
    }

    emit_session_changed(&app, &state);
    emit_tasks_changed(&app);
    crate::google::kick_sync(&app);
    Ok(())
}

/// 直接完了 (spec §4.3): 中断中/未開始のタスクを duration=0 の completed ログで完了させる。
#[tauri::command]
pub fn complete_task_direct(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    task: TaskRef,
    memo: Option<String>,
) -> CmdResult<()> {
    {
        let conn = state.db.lock().unwrap();

        // running 中のタスクは stop_task(completed) を使う。ここでは対象外としてブロック
        if let Some(active) = repos::get_active_session(&conn).map_err(db_err)? {
            if active.task_list_id == task.task_list_id && active.task_id == task.task_id {
                return Err("実行中のタスクは「完了」ボタンで終了してください。".into());
            }
        }

        let row = repos::get_task(&conn, &task.task_list_id, &task.task_id)
            .map_err(db_err)?
            .ok_or("タスクが見つかりません。")?;

        let now = time::to_iso(&time::now_utc());
        let tx = conn.unchecked_transaction().map_err(db_err)?;
        repos::append_work_log(
            &tx,
            &repos::NewWorkLog {
                task_list_id: &row.task_list_id,
                task_list_name: &row.task_list_title,
                task_id: &row.id,
                task_title: &row.title,
                action_type: "completed",
                start_time: now.clone(),
                end_time: now,
                duration_seconds: 0,
                memo: memo.as_deref().unwrap_or(""),
                end_reason: "direct_complete",
            },
        )
        .map_err(db_err)?;
        repos::mark_task_completed_locally(&tx, &row.task_list_id, &row.id).map_err(db_err)?;
        repos::enqueue_sync_op(&tx, "complete_task", &row.task_list_id, &row.id, "{}")
            .map_err(db_err)?;
        tx.commit().map_err(db_err)?;
    }

    emit_tasks_changed(&app);
    crate::google::kick_sync(&app);
    Ok(())
}

/// 今すぐやる (spec §5.3): due=today 化 + 即開始。別タスク running 中はブロック。
#[tauri::command]
pub fn do_it_now(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    task: TaskRef,
) -> CmdResult<()> {
    {
        let conn = state.db.lock().unwrap();

        if repos::get_active_session(&conn).map_err(db_err)?.is_some() {
            return Err(ERR_ALREADY_RUNNING.into());
        }

        let row = repos::get_task(&conn, &task.task_list_id, &task.task_id)
            .map_err(db_err)?
            .ok_or("タスクが見つかりません。")?;

        let tx = conn.unchecked_transaction().map_err(db_err)?;
        repos::set_task_due_today_locally(&tx, &row.task_list_id, &row.id).map_err(db_err)?;
        repos::enqueue_sync_op(&tx, "set_due_today", &row.task_list_id, &row.id, "{}")
            .map_err(db_err)?;
        repos::create_active_session(
            &tx,
            &row.task_list_id,
            &row.task_list_title,
            &row.id,
            &row.title,
        )
        .map_err(db_err)?;
        tx.commit().map_err(db_err)?;
    }

    emit_session_changed(&app, &state);
    emit_tasks_changed(&app);
    crate::google::kick_sync(&app);
    Ok(())
}
