//! Installed applications (Start menu shortcuts) and running applications with windows.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Serialize;
use windows::core::{Interface, BOOL, HSTRING};
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, IPersistFile, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
    STGM_READ,
};
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindow, GetWindowLongPtrW, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, IsWindowVisible, GWL_EXSTYLE, GW_OWNER, WS_EX_TOOLWINDOW,
};

use crate::profiles::store::normalize_exe;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    /// Display name (shortcut name or window title).
    pub name: String,
    /// Lower-case exe file name.
    pub exe: String,
    /// Full path of the executable.
    pub path: String,
}

/// Resolves a `.lnk` shortcut to its target path.
fn resolve_lnk(link: &IShellLinkW, lnk: &Path) -> Option<String> {
    unsafe {
        let pf: IPersistFile = link.cast().ok()?;
        pf.Load(&HSTRING::from(lnk.as_os_str()), STGM_READ).ok()?;
        let mut buf = [0u16; 1024];
        link.GetPath(&mut buf, std::ptr::null_mut(), 0).ok()?;
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        let s = String::from_utf16_lossy(&buf[..len]);
        (!s.is_empty()).then_some(s)
    }
}

fn walk_lnk(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    if depth > 4 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk_lnk(&p, out, depth + 1);
        } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("lnk")) {
            out.push(p);
        }
    }
}

/// Applications from Start menu shortcuts in `%ProgramData%` and `%AppData%` (SPEC 3.1).
pub fn installed_apps() -> Vec<AppInfo> {
    let mut lnks = Vec::new();
    for var in ["ProgramData", "APPDATA"] {
        if let Some(base) = std::env::var_os(var) {
            walk_lnk(
                &PathBuf::from(base).join(r"Microsoft\Windows\Start Menu\Programs"),
                &mut lnks,
                0,
            );
        }
    }
    // Shell links need an STA thread.
    let handle = std::thread::spawn(move || {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
        let Ok(link) =
            (unsafe { CoCreateInstance::<_, IShellLinkW>(&ShellLink, None, CLSCTX_INPROC_SERVER) })
        else {
            return Vec::new();
        };
        let mut seen = HashSet::new();
        let mut apps = Vec::new();
        for lnk in lnks {
            let name = lnk
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let lower = name.to_lowercase();
            if lower.contains("uninstall") || lower.contains("odinstal") {
                continue;
            }
            let Some(target) = resolve_lnk(&link, &lnk) else {
                continue;
            };
            if !target.to_lowercase().ends_with(".exe") || !Path::new(&target).exists() {
                continue;
            }
            let exe = normalize_exe(&target);
            if seen.insert(target.to_lowercase()) {
                apps.push(AppInfo {
                    name,
                    exe,
                    path: target,
                });
            }
        }
        apps.sort_by_key(|a| a.name.to_lowercase());
        apps
    });
    handle.join().unwrap_or_default()
}

unsafe extern "system" fn collect_window(hwnd: HWND, data: LPARAM) -> BOOL {
    let list = &mut *(data.0 as *mut Vec<(HWND, u32, String)>);
    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }
    if GetWindow(hwnd, GW_OWNER).is_ok_and(|o| !o.0.is_null()) {
        return BOOL(1);
    }
    if GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0 != 0 {
        return BOOL(1);
    }
    let len = GetWindowTextLengthW(hwnd);
    if len == 0 {
        return BOOL(1);
    }
    let mut buf = vec![0u16; len as usize + 1];
    let n = GetWindowTextW(hwnd, &mut buf);
    let mut pid = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    list.push((hwnd, pid, String::from_utf16_lossy(&buf[..n as usize])));
    BOOL(1)
}

/// Processes that currently have a visible top-level window (excluding this app).
pub fn running_apps() -> Vec<AppInfo> {
    let mut windows: Vec<(HWND, u32, String)> = Vec::new();
    unsafe {
        let _ = EnumWindows(
            Some(collect_window),
            LPARAM(&mut windows as *mut _ as isize),
        );
    }
    let me = std::process::id();
    let mut seen = HashSet::new();
    let mut apps = Vec::new();
    for (_, pid, title) in windows {
        if pid == me {
            continue;
        }
        let Some(path) = crate::foreground::process_path(pid) else {
            continue;
        };
        let exe = normalize_exe(&path);
        // Shell hosts are not useful targets for app profiles.
        if matches!(
            exe.as_str(),
            "applicationframehost.exe" | "textinputhost.exe" | "shellexperiencehost.exe"
        ) {
            continue;
        }
        if seen.insert(exe.clone()) {
            apps.push(AppInfo {
                name: title,
                exe,
                path,
            });
        }
    }
    apps.sort_by_key(|a| a.exe.clone());
    apps
}
