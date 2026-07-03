//! OS 依存の電源イベント検知 (spec §7.1)。
//! 他 OS 対応時はここに実装を追加する。非対応 OS ではハートビート保険のみで動く。

#[cfg(windows)]
mod win_power;

pub fn start_power_monitor(app: tauri::AppHandle) {
    #[cfg(windows)]
    win_power::start(app);
    #[cfg(not(windows))]
    let _ = app;
}
