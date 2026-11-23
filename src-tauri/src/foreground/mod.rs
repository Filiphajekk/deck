//! Information about the foreground window and restoring focus to it.

pub mod watcher;

use std::time::Duration;

use serde::Serialize;
use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::Threading::{
    AttachThreadInput, GetCurrentProcessId, GetCurrentThreadId, OpenProcess, OpenProcessToken,
    QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, IsIconic, IsWindow,
    SetForegroundWindow, ShowWindow, SW_RESTORE,
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ForegroundInfo {
    /// HWND as an integer so it can cross threads.
    pub hwnd: isize,
    pub pid: u32,
    /// Lower-case exe file name, e.g. `notepad.exe`.
    pub exe: Option<String>,
    pub path: Option<String>,
}

impl ForegroundInfo {
    pub fn hwnd(&self) -> HWND {
        HWND(self.hwnd as *mut _)
    }

    pub fn is_own_process(&self) -> bool {
        self.pid == unsafe { GetCurrentProcessId() }
    }
}

/// Full image path of a process, if we are allowed to query it.
pub fn process_path(pid: u32) -> Option<String> {
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok =
            QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(h);
        ok.ok()?;
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }
}

pub fn exe_name(path: &str) -> String {
    crate::profiles::store::normalize_exe(path)
}

pub fn info_for(hwnd: HWND) -> Option<ForegroundInfo> {
    if hwnd.0.is_null() {
        return None;
    }
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    let path = process_path(pid);
    Some(ForegroundInfo {
        hwnd: hwnd.0 as isize,
        pid,
        exe: path.as_deref().map(exe_name),
        path,
    })
}

pub fn current() -> Option<ForegroundInfo> {
    info_for(unsafe { GetForegroundWindow() })
}

/// Gives focus back to `hwnd` if something else took it (SPEC 4.5 step 3).
pub fn restore_focus(hwnd: HWND) {
    unsafe {
        if hwnd.0.is_null() || !IsWindow(Some(hwnd)).as_bool() {
            return;
        }
        if GetForegroundWindow() == hwnd {
            return;
        }
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        if SetForegroundWindow(hwnd).as_bool() && GetForegroundWindow() == hwnd {
            return;
        }
        // Fallback: attach our input queue to the current foreground thread so the
        // foreground lock lets us switch.
        let fg = GetForegroundWindow();
        let fg_thread = GetWindowThreadProcessId(fg, None);
        let me = GetCurrentThreadId();
        let attached =
            fg_thread != 0 && fg_thread != me && AttachThreadInput(me, fg_thread, true).as_bool();
        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
        if attached {
            let _ = AttachThreadInput(me, fg_thread, false);
        }
        if GetForegroundWindow() != hwnd {
            tracing::warn!("could not restore focus to the target window");
        }
    }
}

/// Waits until `hwnd` is the foreground window (or the timeout passes).
pub fn wait_foreground(hwnd: HWND, timeout: Duration) -> bool {
    let start = std::time::Instant::now();
    loop {
        if unsafe { GetForegroundWindow() } == hwnd {
            return true;
        }
        if start.elapsed() >= timeout {
            return false;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn token_elevated(process: HANDLE) -> Option<bool> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(process, TOKEN_QUERY, &mut token).ok()?;
        let mut elevation = TOKEN_ELEVATION::default();
        let mut len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        );
        let _ = CloseHandle(token);
        ok.ok()?;
        Some(elevation.TokenIsElevated != 0)
    }
}

/// Whether this process runs elevated.
pub fn self_elevated() -> bool {
    token_elevated(unsafe { windows::Win32::System::Threading::GetCurrentProcess() })
        .unwrap_or(false)
}

/// Whether another process runs elevated. If we cannot even open its token while not
/// elevated ourselves, that usually means it is elevated (or a protected process).
pub fn process_elevated(pid: u32) -> bool {
    unsafe {
        let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return true;
        };
        let r = token_elevated(h);
        let _ = CloseHandle(h);
        r.unwrap_or(true)
    }
}

/// True when UIPI will block our synthetic input into `pid` (SPEC 10.2).
pub fn input_blocked_by_uipi(pid: u32) -> bool {
    pid != 0 && !self_elevated() && process_elevated(pid)
}
