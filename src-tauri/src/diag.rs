//! Diagnostics that survive hard failures: a synchronously written `logs\crash.log`
//! (panics and native exceptions) and a watchdog that reports when the UI thread hangs.
//! The normal log is written by a background thread and loses its last lines on a crash.

use std::io::Write;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Diagnostics::Debug::{
    SetUnhandledExceptionFilter, EXCEPTION_CONTINUE_SEARCH, EXCEPTION_POINTERS,
};
use windows::Win32::UI::WindowsAndMessaging::{SendMessageTimeoutW, SMTO_ABORTIFHUNG, WM_NULL};

use crate::paths;

/// Appends a line to `logs\crash.log` right away (no buffering).
pub fn append_crash(text: &str) {
    let path = paths::logs_dir().join("crash.log");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = writeln!(f, "[unix {now}] v{} {}\n", env!("CARGO_PKG_VERSION"), text);
    }
}

unsafe extern "system" fn on_unhandled_exception(info: *const EXCEPTION_POINTERS) -> i32 {
    if let Some(rec) = info.as_ref().and_then(|i| i.ExceptionRecord.as_ref()) {
        append_crash(&format!(
            "native exception {:#010x} at {:?} on thread {:?}",
            rec.ExceptionCode.0 as u32,
            rec.ExceptionAddress,
            std::thread::current().name()
        ));
    }
    EXCEPTION_CONTINUE_SEARCH
}

/// Records native crashes (access violations etc.) in crash.log.
pub fn install_crash_handler() {
    unsafe {
        SetUnhandledExceptionFilter(Some(on_unhandled_exception));
    }
}

static UI_HWND: AtomicIsize = AtomicIsize::new(0);

/// Pings a window of the UI thread every 2 s and logs when it stops answering (the state
/// Windows reports as "not responding"), and how long it took to recover.
pub fn start_ui_watchdog(hwnd: isize) {
    UI_HWND.store(hwnd, Ordering::SeqCst);
    std::thread::Builder::new()
        .name("ui-watchdog".into())
        .spawn(|| {
            let mut hung_since: Option<Instant> = None;
            loop {
                std::thread::sleep(Duration::from_secs(2));
                let h = HWND(UI_HWND.load(Ordering::SeqCst) as *mut _);
                let mut result = 0usize;
                let ok = unsafe {
                    SendMessageTimeoutW(
                        h,
                        WM_NULL,
                        WPARAM(0),
                        LPARAM(0),
                        SMTO_ABORTIFHUNG,
                        3000,
                        Some(&mut result),
                    )
                }
                .0 != 0;
                match (ok, hung_since) {
                    (false, None) => {
                        hung_since = Some(Instant::now());
                        tracing::warn!("UI thread is not responding");
                        append_crash("UI thread is not responding (watchdog)");
                    }
                    (true, Some(t)) => {
                        let s = t.elapsed().as_secs_f32() + 3.0;
                        tracing::warn!("UI thread responds again after ~{s:.0} s");
                        append_crash(&format!("UI thread recovered after ~{s:.0} s"));
                        hung_since = None;
                    }
                    _ => {}
                }
            }
        })
        .ok();
}
