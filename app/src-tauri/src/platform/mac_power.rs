use std::ffi::c_void;
use std::sync::OnceLock;

const K_CG_EVENT_SOURCE_STATE_COMBINED_SESSION_STATE: i32 = 0;
const K_CG_ANY_INPUT_EVENT_TYPE: u32 = !0;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(state_id: i32, event_type: u32) -> f64;
}

pub fn idle_seconds() -> Option<i64> {
    let secs = unsafe {
        CGEventSourceSecondsSinceLastEventType(
            K_CG_EVENT_SOURCE_STATE_COMBINED_SESSION_STATE,
            K_CG_ANY_INPUT_EVENT_TYPE,
        )
    };
    if secs.is_finite() && secs >= 0.0 {
        Some(secs as i64)
    } else {
        None
    }
}

type IoServiceT = u32;
type IoConnectT = u32;
type IoObjectT = u32;

const K_IO_MESSAGE_SYSTEM_WILL_SLEEP: u32 = 0xE0000280;
const K_IO_MESSAGE_CAN_SYSTEM_SLEEP: u32 = 0xE0000270;
const K_IO_MESSAGE_SYSTEM_HAS_POWERED_ON: u32 = 0xE0000300;

#[repr(C)]
struct IoNotificationPort {
    _private: [u8; 0],
}
type IoNotificationPortRef = *mut IoNotificationPort;

type IoServiceInterestCallback =
    unsafe extern "C" fn(refcon: *mut c_void, service: IoServiceT, message_type: u32, message_argument: *mut c_void);

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IORegisterForSystemPower(
        refcon: *mut c_void,
        the_port_ref: *mut IoNotificationPortRef,
        callback: IoServiceInterestCallback,
        notifier: *mut IoObjectT,
    ) -> IoConnectT;
    fn IONotificationPortGetRunLoopSource(notify: IoNotificationPortRef) -> *mut c_void;
    fn IOAllowPowerChange(kernel_port: IoConnectT, notification_id: isize) -> i32;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    static kCFRunLoopDefaultMode: *const c_void;
    fn CFRunLoopGetCurrent() -> *mut c_void;
    fn CFRunLoopAddSource(rl: *mut c_void, source: *mut c_void, mode: *const c_void);
    fn CFRunLoopRun();
}

static APP: OnceLock<tauri::AppHandle> = OnceLock::new();
static ROOT_POWER_PORT: OnceLock<usize> = OnceLock::new();

unsafe extern "C" fn power_callback(
    _refcon: *mut c_void,
    _service: IoServiceT,
    message_type: u32,
    message_argument: *mut c_void,
) {
    let Some(app) = APP.get() else { return };
    match message_type {
        K_IO_MESSAGE_CAN_SYSTEM_SLEEP => {
            if let Some(&port) = ROOT_POWER_PORT.get() {
                IOAllowPowerChange(port as IoConnectT, message_argument as isize);
            }
        }
        K_IO_MESSAGE_SYSTEM_WILL_SLEEP => {
            // 同期的に書き切ってから ack する: OS はこの呼び出しが返るまで(最大約30秒)
            // 実際のサスペンドを待つため、DB 書き込み漏れが起きない。
            crate::power::on_suspend(app);
            if let Some(&port) = ROOT_POWER_PORT.get() {
                IOAllowPowerChange(port as IoConnectT, message_argument as isize);
            }
        }
        K_IO_MESSAGE_SYSTEM_HAS_POWERED_ON => {
            let app = app.clone();
            std::thread::spawn(move || crate::power::on_resume(&app));
        }
        _ => {}
    }
}

pub fn start(app: tauri::AppHandle) {
    if APP.set(app).is_err() {
        return;
    }

    std::thread::spawn(|| unsafe {
        let mut notify_port: IoNotificationPortRef = std::ptr::null_mut();
        let mut notifier: IoObjectT = 0;
        let root_port = IORegisterForSystemPower(
            std::ptr::null_mut(),
            &mut notify_port,
            power_callback,
            &mut notifier,
        );
        if root_port == 0 {
            eprintln!("IORegisterForSystemPower failed");
            return;
        }
        let _ = ROOT_POWER_PORT.set(root_port as usize);

        let source = IONotificationPortGetRunLoopSource(notify_port);
        CFRunLoopAddSource(CFRunLoopGetCurrent(), source, kCFRunLoopDefaultMode);
        CFRunLoopRun();
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn idle_seconds_returns_a_value_on_macos() {
        let idle = super::idle_seconds();
        assert!(idle.is_some());
        assert!(idle.unwrap() >= 0);
    }
}
