//! Launching programs, opening URLs/files and running commands.

use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};

use anyhow::{bail, Context};
use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::Foundation::ERROR_CANCELLED;
use windows::Win32::System::Com::{
    CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
};
use windows::Win32::System::Threading::{CREATE_NEW_CONSOLE, CREATE_NO_WINDOW};
use windows::Win32::UI::Shell::{
    ShellExecuteExW, SEE_MASK_DOENVSUBST, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW,
};
use windows::Win32::UI::WindowsAndMessaging::{AllowSetForegroundWindow, ASFW_ANY, SW_SHOWNORMAL};

use crate::profiles::model::Shell;

/// Runs `f` on a fresh STA thread (the shell expects STA for ShellExecuteEx).
fn run_sta<F>(f: F) -> anyhow::Result<()>
where
    F: FnOnce() -> anyhow::Result<()> + Send + 'static,
{
    std::thread::Builder::new()
        .name("shell-sta".into())
        .spawn(move || unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
            let r = f();
            CoUninitialize();
            r
        })?
        .join()
        .map_err(|_| anyhow::anyhow!("shell thread panicked"))?
}

fn shell_execute(file: &str, params: &str, dir: &str, runas: bool) -> anyhow::Result<()> {
    if file.trim().is_empty() {
        bail!("nothing to open");
    }
    let file = HSTRING::from(file.trim());
    let params = HSTRING::from(params);
    let dir = HSTRING::from(dir);
    run_sta(move || unsafe {
        // Let the started program take the foreground even though we are not focused.
        let _ = AllowSetForegroundWindow(ASFW_ANY);
        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI | SEE_MASK_DOENVSUBST,
            lpVerb: if runas { w!("runas") } else { PCWSTR::null() },
            lpFile: PCWSTR(file.as_ptr()),
            lpParameters: if params.is_empty() {
                PCWSTR::null()
            } else {
                PCWSTR(params.as_ptr())
            },
            lpDirectory: if dir.is_empty() {
                PCWSTR::null()
            } else {
                PCWSTR(dir.as_ptr())
            },
            nShow: SW_SHOWNORMAL.0,
            ..Default::default()
        };
        match ShellExecuteExW(&mut info) {
            Ok(()) => Ok(()),
            // The user declined the UAC prompt: not an error.
            Err(e) if e.code() == ERROR_CANCELLED.to_hresult() => Ok(()),
            Err(e) => Err(anyhow::Error::from(e).context(format!("cannot open {file}"))),
        }
    })
}

pub fn launch(path: &str, args: &str, working_dir: &str, admin: bool) -> anyhow::Result<()> {
    shell_execute(path, args, working_dir, admin)
}

/// URL, file, folder or `ms-settings:` URI.
pub fn open(target: &str) -> anyhow::Result<()> {
    shell_execute(target, "", "", false)
}

pub fn command(command: &str, shell: Shell, hidden: bool) -> anyhow::Result<()> {
    if command.trim().is_empty() {
        bail!("empty command");
    }
    let mut cmd = match shell {
        Shell::Cmd => {
            let mut c = Command::new("cmd.exe");
            // raw_arg keeps the command exactly as typed (cmd has its own quoting rules).
            c.raw_arg("/C").raw_arg(command);
            c
        }
        Shell::Powershell => {
            let mut c = Command::new("powershell.exe");
            c.args(["-NoProfile", "-Command", command]);
            c
        }
    };
    if hidden {
        cmd.creation_flags(CREATE_NO_WINDOW.0)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
    } else {
        cmd.creation_flags(CREATE_NEW_CONSOLE.0);
    }
    cmd.spawn().context("cannot start the command")?;
    Ok(())
}
