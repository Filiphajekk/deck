//! Per-application volume through `IAudioSessionManager2` / `ISimpleAudioVolume`.

use windows::core::Interface;
use windows::Win32::Media::Audio::{
    eConsole, eRender, IAudioSessionControl2, IAudioSessionManager2, ISimpleAudioVolume,
};
use windows::Win32::System::Com::CLSCTX_ALL;

use crate::profiles::store::normalize_exe;

/// All session volumes of the default output device that belong to `exe`.
fn volumes_for(exe: &str) -> anyhow::Result<Vec<ISimpleAudioVolume>> {
    let exe = normalize_exe(exe);
    let mut out = Vec::new();
    unsafe {
        let dev = super::enumerator()?.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let mgr: IAudioSessionManager2 = dev.Activate(CLSCTX_ALL, None)?;
        let sessions = mgr.GetSessionEnumerator()?;
        for i in 0..sessions.GetCount()? {
            let ctl = sessions.GetSession(i)?;
            let Ok(ctl2) = ctl.cast::<IAudioSessionControl2>() else {
                continue;
            };
            let Ok(pid) = ctl2.GetProcessId() else {
                continue;
            };
            let name = crate::foreground::process_path(pid).map(|p| normalize_exe(&p));
            if name.as_deref() == Some(exe.as_str()) {
                if let Ok(v) = ctl.cast::<ISimpleAudioVolume>() {
                    out.push(v);
                }
            }
        }
    }
    Ok(out)
}

fn require(exe: &str) -> anyhow::Result<Vec<ISimpleAudioVolume>> {
    let v = volumes_for(exe)?;
    anyhow::ensure!(!v.is_empty(), "{exe} is not playing any audio right now");
    Ok(v)
}

pub fn change_app_volume(exe: &str, delta: f32) -> anyhow::Result<()> {
    for v in require(exe)? {
        unsafe {
            let level = (v.GetMasterVolume()? + delta).clamp(0.0, 1.0);
            v.SetMasterVolume(level, std::ptr::null())?;
        }
    }
    Ok(())
}

pub fn toggle_app_mute(exe: &str) -> anyhow::Result<bool> {
    let vols = require(exe)?;
    let muted = unsafe { !vols[0].GetMute()?.as_bool() };
    for v in vols {
        unsafe { v.SetMute(muted, std::ptr::null())? };
    }
    Ok(muted)
}

pub fn app_muted(exe: &str) -> bool {
    volumes_for(exe)
        .ok()
        .and_then(|v| v.first().and_then(|v| unsafe { v.GetMute() }.ok()))
        .is_some_and(|m| m.as_bool())
}

/// Exe names of processes that currently have an audio session (for the editor picker).
pub fn apps_with_sessions() -> Vec<String> {
    let mut out = Vec::new();
    unsafe {
        let Ok(en) = super::enumerator() else {
            return out;
        };
        let Ok(dev) = en.GetDefaultAudioEndpoint(eRender, eConsole) else {
            return out;
        };
        let Ok(mgr) = dev.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else {
            return out;
        };
        let Ok(sessions) = mgr.GetSessionEnumerator() else {
            return out;
        };
        for i in 0..sessions.GetCount().unwrap_or(0) {
            let Ok(ctl) = sessions.GetSession(i) else {
                continue;
            };
            let Ok(ctl2) = ctl.cast::<IAudioSessionControl2>() else {
                continue;
            };
            if let Ok(pid) = ctl2.GetProcessId() {
                if pid == 0 {
                    continue;
                }
                if let Some(p) = crate::foreground::process_path(pid) {
                    let n = normalize_exe(&p);
                    if !out.contains(&n) {
                        out.push(n);
                    }
                }
            }
        }
    }
    out
}
