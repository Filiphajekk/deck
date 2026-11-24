//! The overlay window: created once (hidden) and then only moved, shown and hidden via Win32.
//!
//! Position/show/hide are posted asynchronously (`SWP_ASYNCWINDOWPOS`, `ShowWindowAsync`)
//! so the overlay controller never waits for the UI thread, even if it is busy.
//!
//! It never takes focus (`WS_EX_NOACTIVATE`, shown with `SW_SHOWNOACTIVATE`) and ignores the
//! mouse (click-through); all interaction is handled by the input hooks.

use tauri::{AppHandle, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, ShowWindowAsync, GWL_EXSTYLE, HWND_TOPMOST,
    SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOOWNERZORDER, SWP_SHOWWINDOW, SW_HIDE,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};

use super::geometry::Rect;

pub const OVERLAY_LABEL: &str = "overlay";

pub fn create(app: &AppHandle) -> tauri::Result<(WebviewWindow, isize)> {
    let win = WebviewWindowBuilder::new(app, OVERLAY_LABEL, WebviewUrl::App("overlay.html".into()))
        .title("Deckless overlay")
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .shadow(false)
        .resizable(false)
        .visible(false)
        .inner_size(380.0, 380.0)
        .position(-20000.0, -20000.0)
        .build()?;
    let hwnd = win.hwnd()?;
    unsafe {
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(
            hwnd,
            GWL_EXSTYLE,
            ex | WS_EX_NOACTIVATE.0 as isize | WS_EX_TOOLWINDOW.0 as isize,
        );
    }
    win.set_ignore_cursor_events(true)?;
    Ok((win, hwnd.0 as isize))
}

fn hwnd(h: isize) -> HWND {
    HWND(h as *mut _)
}

#[derive(Debug, Clone, Copy)]
pub struct MonitorAt {
    pub work: Rect,
    pub scale: f64,
}

/// Work area and DPI scale of the monitor under a point (physical coordinates).
pub fn monitor_at(x: i32, y: i32) -> MonitorAt {
    unsafe {
        let mon = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let _ = GetMonitorInfoW(mon, &mut mi);
        let (mut dx, mut dy) = (96u32, 96u32);
        let _ = GetDpiForMonitor(mon, MDT_EFFECTIVE_DPI, &mut dx, &mut dy);
        MonitorAt {
            work: Rect {
                left: mi.rcWork.left,
                top: mi.rcWork.top,
                right: mi.rcWork.right,
                bottom: mi.rcWork.bottom,
            },
            scale: dx as f64 / 96.0,
        }
    }
}

/// Moves the (hidden) window to `rect`. Called twice because moving onto a monitor with a
/// different DPI makes the window resize itself on WM_DPICHANGED.
pub fn position(h: isize, rect: Rect) {
    for _ in 0..2 {
        unsafe {
            let _ = SetWindowPos(
                hwnd(h),
                Some(HWND_TOPMOST),
                rect.left,
                rect.top,
                rect.width(),
                rect.height(),
                SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_ASYNCWINDOWPOS,
            );
        }
    }
}

/// Shows the window on top of everything without activating it.
pub fn show(h: isize, rect: Rect) {
    unsafe {
        let _ = SetWindowPos(
            hwnd(h),
            Some(HWND_TOPMOST),
            rect.left,
            rect.top,
            rect.width(),
            rect.height(),
            SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_NOOWNERZORDER | SWP_ASYNCWINDOWPOS,
        );
    }
}

pub fn hide(h: isize) {
    unsafe {
        let _ = ShowWindowAsync(hwnd(h), SW_HIDE);
    }
}
