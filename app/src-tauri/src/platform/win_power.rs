//! Windows のスリープ/復帰検知。
//! PowerRegisterSuspendResumeNotification (powrprof) の DEVICE_NOTIFY_CALLBACK 方式:
//! 隠しウィンドウ + WM_POWERBROADCAST が不要で、Modern Standby でも通知される (spec §7.1)。
//! windows クレートは使わず最小限の手書き FFI (依存とバージョン差異を避ける)。

use std::ffi::c_void;
use std::sync::OnceLock;

const DEVICE_NOTIFY_CALLBACK: u32 = 2;
// winuser.h の PBT_* 定数
const PBT_APMSUSPEND: u32 = 0x0004;
const PBT_APMRESUMESUSPEND: u32 = 0x0007;
const PBT_APMRESUMEAUTOMATIC: u32 = 0x0012;

#[repr(C)]
struct DeviceNotifySubscribeParameters {
    callback: unsafe extern "system" fn(*mut c_void, u32, *mut c_void) -> u32,
    context: *mut c_void,
}

#[link(name = "powrprof")]
extern "system" {
    fn PowerRegisterSuspendResumeNotification(
        flags: u32,
        recipient: *mut c_void,
        registration_handle: *mut *mut c_void,
    ) -> u32;
}

#[repr(C)]
struct LastInputInfo {
    cb_size: u32,
    dw_time: u32,
}

#[link(name = "user32")]
extern "system" {
    fn GetLastInputInfo(plii: *mut LastInputInfo) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetTickCount() -> u32;
}

/// 最後のキーボード/マウス入力からの経過秒 (spec §7.1 無操作検知)。
/// GetTickCount はスリープ中も進むため、スリープを挟んだ無操作も正しく計測できる。
pub fn idle_seconds() -> Option<i64> {
    unsafe {
        let mut info = LastInputInfo {
            cb_size: std::mem::size_of::<LastInputInfo>() as u32,
            dw_time: 0,
        };
        if GetLastInputInfo(&mut info) == 0 {
            return None;
        }
        // tick は 49.7 日で一周するため wrapping_sub
        let idle_ms = GetTickCount().wrapping_sub(info.dw_time);
        Some((idle_ms / 1000) as i64)
    }
}

static APP: OnceLock<tauri::AppHandle> = OnceLock::new();

unsafe extern "system" fn power_callback(
    _context: *mut c_void,
    event_type: u32,
    _setting: *mut c_void,
) -> u32 {
    // コールバックはシステムスレッドから呼ばれるため、重い処理は別スレッドへ
    if let Some(app) = APP.get() {
        let app = app.clone();
        match event_type {
            PBT_APMSUSPEND => {
                std::thread::spawn(move || crate::power::on_suspend(&app));
            }
            PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND => {
                std::thread::spawn(move || crate::power::on_resume(&app));
            }
            _ => {}
        }
    }
    0
}

#[cfg(test)]
mod tests {
    #[test]
    fn idle_seconds_returns_a_value_on_windows() {
        let idle = super::idle_seconds();
        assert!(idle.is_some(), "GetLastInputInfo FFI が失敗");
        assert!(idle.unwrap() >= 0);
    }
}

pub fn start(app: tauri::AppHandle) {
    if APP.set(app).is_err() {
        return; // 二重登録防止
    }

    // 登録が生きている間 parameters は有効であり続ける必要があるため leak する
    let params = Box::leak(Box::new(DeviceNotifySubscribeParameters {
        callback: power_callback,
        context: std::ptr::null_mut(),
    }));

    let mut handle: *mut c_void = std::ptr::null_mut();
    let result = unsafe {
        PowerRegisterSuspendResumeNotification(
            DEVICE_NOTIFY_CALLBACK,
            params as *mut DeviceNotifySubscribeParameters as *mut c_void,
            &mut handle,
        )
    };
    if result != 0 {
        // 登録失敗してもハートビート保険 (power::spawn_heartbeat) が動くため続行
        eprintln!("PowerRegisterSuspendResumeNotification failed: {result}");
    }
}
