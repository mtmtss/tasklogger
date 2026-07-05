//! OS 依存の電源イベント検知 (spec §7.1)。
//! 他 OS 対応時はここに実装を追加する。非対応 OS ではハートビート保険のみで動く。

#[cfg(windows)]
mod win_power;
#[cfg(target_os = "macos")]
mod mac_power;

pub fn start_power_monitor(app: tauri::AppHandle) {
    #[cfg(windows)]
    win_power::start(app);
    #[cfg(target_os = "macos")]
    mac_power::start(app);
    #[cfg(not(any(windows, target_os = "macos")))]
    let _ = app;
}

/// 最後のユーザー入力からの経過秒。取得できない環境では None (無操作検知は無効になる)。
pub fn idle_seconds() -> Option<i64> {
    #[cfg(windows)]
    {
        win_power::idle_seconds()
    }
    #[cfg(target_os = "macos")]
    {
        mac_power::idle_seconds()
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        None
    }
}
