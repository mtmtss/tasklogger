mod commands;
mod db;
mod domain;
mod google;
mod state;

use rusqlite::Connection;
use tauri::Manager;

use db::repos;
use domain::time;
use state::AppState;

/// フロートウィンドウの表示/非表示を切り替える。戻り値は切替後の表示状態。
#[tauri::command]
fn toggle_float_window(app: tauri::AppHandle) -> Result<bool, String> {
    let window = app
        .get_webview_window("float")
        .ok_or("float window not found")?;
    let visible = window.is_visible().map_err(|e| e.to_string())?;
    if visible {
        window.hide().map_err(|e| e.to_string())?;
    } else {
        window.show().map_err(|e| e.to_string())?;
    }
    Ok(!visible)
}

/// 起動時回復 (spec §7.3): active_session が残っていたら
/// end_time=last_heartbeat_at で recovery ログを書いてセッションを消す。
/// 再開ダイアログの表示は M3 (sleep_interrupted_task 設定だけ先に残す)。
fn recover_orphan_session(conn: &Connection) {
    let session = match conn.query_row(
        "SELECT task_list_id, task_list_name, task_id, task_title, start_at, last_heartbeat_at
         FROM active_session WHERE id = 1",
        [],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        },
    ) {
        Ok(s) => s,
        Err(_) => return,
    };
    let (task_list_id, task_list_name, task_id, task_title, start_at, heartbeat) = session;

    let start = time::parse_iso(&start_at);
    let end = time::parse_iso(&heartbeat).or(start);
    let duration = match (start, end) {
        (Some(s), Some(e)) => (e - s).num_seconds().max(0),
        _ => 0,
    };

    let _ = repos::append_work_log(
        conn,
        &repos::NewWorkLog {
            task_list_id: &task_list_id,
            task_list_name: &task_list_name,
            task_id: &task_id,
            task_title: &task_title,
            action_type: "paused",
            start_time: start_at,
            end_time: heartbeat,
            duration_seconds: duration,
            memo: "",
            end_reason: "recovery",
        },
    );
    let _ = repos::clear_active_session(conn);
    let _ = repos::set_setting(
        conn,
        "sleep_interrupted_task",
        &serde_json::json!({
            "taskListId": task_list_id,
            "taskId": task_id,
            "taskTitle": task_title,
        })
        .to_string(),
    );
}

/// running 中は 30 秒毎に last_heartbeat_at を更新する (spec §7.1 二次検知の下地)。
fn spawn_heartbeat(app: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(30));
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap();
        let _ = conn.execute(
            "UPDATE active_session SET last_heartbeat_at = ?1 WHERE id = 1",
            rusqlite::params![time::to_iso(&time::now_utc())],
        );
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 二重起動時は既存のメインウィンドウを前面に出す (spec §10)
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let conn = db::open(&data_dir.join("tasklogger.db"))?;
            recover_orphan_session(&conn);
            app.manage(AppState::new(conn));
            spawn_heartbeat(app.handle().clone());
            google::start_sync_worker(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            toggle_float_window,
            commands::session::start_task,
            commands::session::stop_task,
            commands::session::complete_task_direct,
            commands::session::do_it_now,
            commands::dashboard::get_today_dashboard,
            commands::dashboard::get_candidates,
            commands::settings::get_settings,
            commands::settings::set_setting,
            commands::settings::seed_sample_data,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
