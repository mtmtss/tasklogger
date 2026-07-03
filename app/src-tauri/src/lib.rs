use tauri::Manager;

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![toggle_float_window])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
