//! `window` action: operations on the window that was active when the panel opened.

use anyhow::bail;
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, MonitorFromWindow, HDC, HMONITOR, MONITORINFO,
    MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, GetWindowRect, IsWindow, IsZoomed, PostMessageW, SetWindowPos, ShowWindow,
    GWL_EXSTYLE, HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE, WM_CLOSE, WS_EX_TOPMOST,
};

use crate::profiles::model::WindowOp;

pub fn apply(hwnd: HWND, op: WindowOp) -> anyhow::Result<()> {
    unsafe {
        if hwnd.0.is_null() || !IsWindow(Some(hwnd)).as_bool() {
            bail!("the target window no longer exists");
        }
        match op {
            WindowOp::Minimize => {
                let _ = ShowWindow(hwnd, SW_MINIMIZE);
            }
            WindowOp::Maximize => {
                let cmd = if IsZoomed(hwnd).as_bool() {
                    SW_RESTORE
                } else {
                    SW_MAXIMIZE
                };
                let _ = ShowWindow(hwnd, cmd);
            }
            WindowOp::TopmostToggle => {
                let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
                let after = if ex & WS_EX_TOPMOST.0 != 0 {
                    HWND_NOTOPMOST
                } else {
                    HWND_TOPMOST
                };
                SetWindowPos(
                    hwnd,
                    Some(after),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                )?;
            }
            WindowOp::Close => {
                PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0))?;
            }
            WindowOp::NextMonitor => move_to_next_monitor(hwnd)?,
        }
    }
    Ok(())
}

unsafe extern "system" fn collect(mon: HMONITOR, _hdc: HDC, _rc: *mut RECT, data: LPARAM) -> BOOL {
    let list = &mut *(data.0 as *mut Vec<(HMONITOR, RECT)>);
    let mut mi = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if GetMonitorInfoW(mon, &mut mi).as_bool() {
        list.push((mon, mi.rcWork));
    }
    BOOL(1)
}

/// Moves the window to the next monitor (left-to-right order), keeping its relative
/// position and size within the work area.
fn move_to_next_monitor(hwnd: HWND) -> anyhow::Result<()> {
    unsafe {
        let mut monitors: Vec<(HMONITOR, RECT)> = Vec::new();
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(collect),
            LPARAM(&mut monitors as *mut _ as isize),
        );
        if monitors.len() < 2 {
            bail!("only one monitor is connected");
        }
        monitors.sort_by_key(|(_, r)| (r.left, r.top));
        let current = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let idx = monitors
            .iter()
            .position(|(m, _)| *m == current)
            .unwrap_or(0);
        let from = monitors[idx].1;
        let to = monitors[(idx + 1) % monitors.len()].1;

        let was_max = IsZoomed(hwnd).as_bool();
        if was_max {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        let mut wr = RECT::default();
        GetWindowRect(hwnd, &mut wr)?;
        let (fw, fh) = (
            (from.right - from.left) as f64,
            (from.bottom - from.top) as f64,
        );
        let (tw, th) = ((to.right - to.left) as f64, (to.bottom - to.top) as f64);
        let rx = (wr.left - from.left) as f64 / fw;
        let ry = (wr.top - from.top) as f64 / fh;
        let w = (((wr.right - wr.left) as f64 / fw) * tw).min(tw);
        let h = (((wr.bottom - wr.top) as f64 / fh) * th).min(th);
        let x = to.left + (rx * tw) as i32;
        let y = to.top + (ry * th) as i32;
        SetWindowPos(
            hwnd,
            None,
            x,
            y,
            w as i32,
            h as i32,
            SWP_NOZORDER | SWP_NOACTIVATE,
        )?;
        if was_max {
            let _ = ShowWindow(hwnd, SW_MAXIMIZE);
        }
    }
    Ok(())
}
