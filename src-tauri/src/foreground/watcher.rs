//! Watches foreground window changes with `SetWinEventHook(EVENT_SYSTEM_FOREGROUND)`.

use std::sync::OnceLock;

use crossbeam_channel::{Receiver, Sender};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, TranslateMessage, EVENT_SYSTEM_FOREGROUND, MSG,
    WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
};

use super::ForegroundInfo;

static SENDER: OnceLock<Sender<ForegroundInfo>> = OnceLock::new();

unsafe extern "system" fn on_event(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    let _ = std::panic::catch_unwind(|| {
        if let Some(info) = super::info_for(hwnd) {
            if let Some(tx) = SENDER.get() {
                let _ = tx.send(info);
            }
        }
    });
}

/// Starts the watcher thread. Events from our own process (overlay, editor) are skipped.
pub fn start() -> Receiver<ForegroundInfo> {
    let (tx, rx) = crossbeam_channel::unbounded();
    let _ = SENDER.set(tx);
    std::thread::Builder::new()
        .name("foreground-watcher".into())
        .spawn(|| unsafe {
            let hook = SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                None,
                Some(on_event),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            );
            if hook.is_invalid() {
                tracing::error!("SetWinEventHook failed; app profiles follow the panel only");
                return;
            }
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            let _ = UnhookWinEvent(hook);
        })
        .expect("spawn foreground watcher");
    rx
}
