mod commands;
mod db;
mod domain;
mod google;
mod platform;
mod power;
mod state;
mod tray;

use std::sync::Mutex;
use std::time::{Duration, Instant};

use rusqlite::Connection;
use tauri::{Manager, PhysicalPosition};

use db::repos;
use domain::time;
use state::AppState;

/// 起動時回復 (spec §7.3): active_session が残っていたら (= 前回異常終了)
/// end_time=last_heartbeat_at で recovery ログを書いてセッションを消し、
/// 復帰ダイアログの対象として記録する。
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
        power::INTERRUPTED_KEY,
        &serde_json::json!({
            "taskListId": task_list_id,
            "taskId": task_id,
            "taskTitle": task_title,
        })
        .to_string(),
    );
}

/// フロート窓の保存位置を復元する。
fn restore_float_position(app: &tauri::AppHandle) {
    let position = {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap();
        repos::get_setting(&conn, "float_window_position")
            .ok()
            .flatten()
    };
    if let Some(text) = position {
        if let Some((x, y)) = text.split_once(',') {
            if let (Ok(x), Ok(y)) = (x.parse::<i32>(), y.parse::<i32>()) {
                if let Some(window) = app.get_webview_window("float") {
                    let _ = window.set_position(PhysicalPosition::new(x, y));
                }
            }
        }
    }
}

static LAST_FLOAT_SAVE: Mutex<Option<Instant>> = Mutex::new(None);

/// フロート窓の移動を settings に保存 (ドラッグ中の連続イベントは 500ms 間隔に間引く)。
fn save_float_position(app: &tauri::AppHandle, x: i32, y: i32) {
    {
        let mut last = LAST_FLOAT_SAVE.lock().unwrap();
        if let Some(t) = *last {
            if t.elapsed() < Duration::from_millis(500) {
                return;
            }
        }
        *last = Some(Instant::now());
    }
    let state = app.state::<AppState>();
    let conn = state.db.lock().unwrap();
    let _ = repos::set_setting(&conn, "float_window_position", &format!("{x},{y}"));
}

fn close_to_tray_enabled(app: &tauri::AppHandle) -> bool {
    let state = app.state::<AppState>();
    let conn = state.db.lock().unwrap();
    repos::get_setting(&conn, "close_to_tray")
        .ok()
        .flatten()
        .map(|v| v != "false")
        .unwrap_or(true)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 二重起動時は既存のメインウィンドウを前面に出す (spec §10)
            tray::show_main_window(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let conn = db::open(&data_dir.join("tasklogger.db"))?;
            recover_orphan_session(&conn);
            app.manage(AppState::new(conn));

            power::spawn_heartbeat(app.handle().clone());
            platform::start_power_monitor(app.handle().clone());
            google::init(app.handle());
            tray::setup(app.handle())?;
            restore_float_position(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| match event {
            // メイン窓の「閉じる」はトレイ常駐 (設定で無効化可, spec §8.2)
            tauri::WindowEvent::CloseRequested { api, .. } if window.label() == "main" => {
                if close_to_tray_enabled(window.app_handle()) {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
            tauri::WindowEvent::Moved(position) if window.label() == "float" => {
                save_float_position(window.app_handle(), position.x, position.y);
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            commands::toggle_float_window,
            commands::session::start_task,
            commands::session::stop_task,
            commands::session::complete_task_direct,
            commands::session::do_it_now,
            commands::session::get_interrupted_task,
            commands::session::resume_interrupted,
            commands::session::dismiss_interrupted,
            commands::dashboard::get_today_dashboard,
            commands::dashboard::get_candidates,
            commands::analytics::get_archive_analytics,
            commands::import_export::export_csv,
            commands::import_export::import_gas_csv,
            commands::settings::get_settings,
            commands::settings::set_setting,
            commands::settings::seed_sample_data,
            commands::settings::set_autostart,
            google::connect_google,
            google::disconnect_google,
            google::sync_now,
            google::get_sync_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
