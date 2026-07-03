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
