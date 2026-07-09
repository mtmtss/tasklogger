//! システムトレイ (spec §8.4)。左クリック = メイン窓、メニュー = フロート/同期/終了。

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;

use crate::db::repos;
use crate::state::AppState;

pub fn setup(app: &tauri::AppHandle) -> tauri::Result<()> {
    let show_main = MenuItem::with_id(app, "show_main", "メインウィンドウを開く", true, None::<&str>)?;
    let capture = MenuItem::with_id(app, "capture", "思いつきをメモ", true, None::<&str>)?;
    let toggle_float = MenuItem::with_id(app, "toggle_float", "フロートウィンドウ切替", true, None::<&str>)?;
    let sync_now = MenuItem::with_id(app, "sync_now", "今すぐ同期", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "終了", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_main, &capture, &toggle_float, &sync_now, &quit])?;

    TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().expect("icon").clone())
        .tooltip("TaskLogger")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show_main" => show_main_window(app),
            "capture" => crate::commands::show_capture_window(app),
            "toggle_float" => {
                let _ = crate::commands::toggle_float_window(app.clone());
            }
            "sync_now" => crate::google::kick_sync(app),
            "quit" => quit_with_confirm(app.clone()),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

pub fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// 終了 (spec §7.4): running セッションがあれば「中断して終了しますか？」を確認する。
fn quit_with_confirm(app: tauri::AppHandle) {
    // blocking_show をメインスレッドで呼ぶとデッドロックするため別スレッド
    std::thread::spawn(move || {
        let has_session = {
            let state = app.state::<AppState>();
            let conn = state.db.lock().unwrap();
            matches!(repos::get_active_session(&conn), Ok(Some(_)))
        };

        if has_session {
            use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
            let confirmed = app
                .dialog()
                .message("作業中のタスクがあります。中断して終了しますか？")
                .title("TaskLogger")
                .kind(MessageDialogKind::Warning)
                .buttons(MessageDialogButtons::OkCancelCustom(
                    "中断して終了".to_string(),
                    "キャンセル".to_string(),
                ))
                .blocking_show();
            if !confirmed {
                return;
            }
            // 手動終了による中断: 復帰ダイアログの対象にはしない
            crate::power::auto_pause(&app, None, "user", false);
        }
        app.exit(0);
    });
}
