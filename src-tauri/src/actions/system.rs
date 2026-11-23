//! Lock and sleep.

use anyhow::bail;
use windows::core::w;
use windows::Win32::Foundation::{CloseHandle, HANDLE, LUID};
use windows::Win32::Security::{
    AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED,
    TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows::Win32::System::Power::SetSuspendState;
use windows::Win32::System::Shutdown::LockWorkStation;
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// Locks the workstation (also used for Win+L, which cannot be injected).
pub fn lock() -> anyhow::Result<()> {
    unsafe { LockWorkStation()? };
    Ok(())
}

/// SetSuspendState needs SE_SHUTDOWN_NAME enabled in our token.
fn enable_shutdown_privilege() -> anyhow::Result<()> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut token,
        )?;
        let mut luid = LUID::default();
        let result =
            LookupPrivilegeValueW(None, w!("SeShutdownPrivilege"), &mut luid).and_then(|_| {
                let tp = TOKEN_PRIVILEGES {
                    PrivilegeCount: 1,
                    Privileges: [LUID_AND_ATTRIBUTES {
                        Luid: luid,
                        Attributes: SE_PRIVILEGE_ENABLED,
                    }],
                };
                AdjustTokenPrivileges(token, false, Some(&tp), 0, None, None)
            });
        let _ = CloseHandle(token);
        result?;
    }
    Ok(())
}

/// Puts the computer to sleep (not hibernate).
pub fn sleep() -> anyhow::Result<()> {
    enable_shutdown_privilege()?;
    let ok = unsafe { SetSuspendState(false, false, false) };
    if !ok {
        bail!(
            "the system refused to sleep: {}",
            std::io::Error::last_os_error()
        );
    }
    Ok(())
}
