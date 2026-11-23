//! "Run as administrator" (SPEC 8, 10.2): restarting through the normal UAC prompt.
//! Nothing here bypasses UAC; if the user declines, the app keeps running unelevated.

use std::time::Duration;

use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, ERROR_CANCELLED, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE};
use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

pub use crate::foreground::self_elevated as is_elevated;

/// Starts a new elevated instance of this exe. Returns Ok(false) if the user declined UAC.
pub fn relaunch_elevated(extra_args: &[&str]) -> anyhow::Result<bool> {
    let exe = std::env::current_exe()?;
    let mut args = format!("--wait-pid {}", std::process::id());
    for a in extra_args {
        args.push(' ');
        args.push_str(a);
    }
    let file = HSTRING::from(exe.as_os_str());
    let params = HSTRING::from(args);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOASYNC,
        lpVerb: w!("runas"),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(params.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    match unsafe { ShellExecuteExW(&mut info) } {
        Ok(()) => Ok(true),
        Err(e) if e.code() == ERROR_CANCELLED.to_hresult() => Ok(false),
        Err(e) => Err(e.into()),
    }
}

/// When started with `--wait-pid <pid>`, waits (max 10 s) for the previous instance to exit
/// so that the single-instance check does not hand over to it.
pub fn wait_for_previous_instance() {
    let args: Vec<String> = std::env::args().collect();
    let Some(pid) = args
        .iter()
        .position(|a| a == "--wait-pid")
        .and_then(|i| args.get(i + 1))
        .and_then(|p| p.parse::<u32>().ok())
    else {
        return;
    };
    unsafe {
        if let Ok(h) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
            let r = WaitForSingleObject(h, Duration::from_secs(10).as_millis() as u32);
            if r != WAIT_OBJECT_0 {
                tracing::warn!(pid, "previous instance did not exit in time");
            }
            let _ = CloseHandle(h);
        }
    }
}
