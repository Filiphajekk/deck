//! Core Audio: master volume/mute, microphone mute, per-app volume, default devices.

pub mod policy;
pub mod sessions;

use std::cell::Cell;

use anyhow::Context;
use serde::Serialize;
use windows::core::PWSTR;
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    eCapture, eCommunications, eConsole, eRender, EDataFlow, ERole, IMMDevice, IMMDeviceEnumerator,
    MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED, STGM_READ,
};

use crate::profiles::model::AudioFlow;

thread_local! {
    static COM_READY: Cell<bool> = const { Cell::new(false) };
}

/// Initialises COM (MTA) once per thread. Harmless if the thread already has an apartment.
pub fn com_init() {
    COM_READY.with(|r| {
        if !r.get() {
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            }
            r.set(true);
        }
    });
}

pub(crate) fn enumerator() -> anyhow::Result<IMMDeviceEnumerator> {
    com_init();
    unsafe { Ok(CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?) }
}

fn flow_of(flow: AudioFlow) -> EDataFlow {
    match flow {
        AudioFlow::Output => eRender,
        AudioFlow::Input => eCapture,
    }
}

fn default_device(flow: EDataFlow, role: ERole) -> anyhow::Result<IMMDevice> {
    unsafe {
        enumerator()?
            .GetDefaultAudioEndpoint(flow, role)
            .context("no default audio device")
    }
}

fn endpoint_volume(flow: EDataFlow, role: ERole) -> anyhow::Result<IAudioEndpointVolume> {
    let dev = default_device(flow, role)?;
    unsafe { Ok(dev.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)?) }
}

fn speakers() -> anyhow::Result<IAudioEndpointVolume> {
    endpoint_volume(eRender, eConsole)
}

/// Default recording device for communications (what Discord, Teams and games use).
fn microphone() -> anyhow::Result<IAudioEndpointVolume> {
    endpoint_volume(eCapture, eCommunications)
}

/// Changes the master volume by `delta` (-1.0..1.0). Unmutes when turning up.
pub fn change_volume(delta: f32) -> anyhow::Result<f32> {
    let ev = speakers()?;
    unsafe {
        let v = (ev.GetMasterVolumeLevelScalar()? + delta).clamp(0.0, 1.0);
        ev.SetMasterVolumeLevelScalar(v, std::ptr::null())?;
        if delta > 0.0 && ev.GetMute()?.as_bool() {
            ev.SetMute(false, std::ptr::null())?;
        }
        Ok(v)
    }
}

pub fn set_volume(level: f32) -> anyhow::Result<()> {
    let ev = speakers()?;
    unsafe {
        ev.SetMasterVolumeLevelScalar(level.clamp(0.0, 1.0), std::ptr::null())?;
    }
    Ok(())
}

pub fn output_muted() -> anyhow::Result<bool> {
    unsafe { Ok(speakers()?.GetMute()?.as_bool()) }
}

/// Toggles master mute; returns the new state.
pub fn toggle_output_mute() -> anyhow::Result<bool> {
    let ev = speakers()?;
    unsafe {
        let m = !ev.GetMute()?.as_bool();
        ev.SetMute(m, std::ptr::null())?;
        Ok(m)
    }
}

pub fn mic_muted() -> anyhow::Result<bool> {
    unsafe { Ok(microphone()?.GetMute()?.as_bool()) }
}

/// Toggles the microphone mute; returns the new state.
pub fn toggle_mic_mute() -> anyhow::Result<bool> {
    let ev = microphone()?;
    unsafe {
        let m = !ev.GetMute()?.as_bool();
        ev.SetMute(m, std::ptr::null())?;
        Ok(m)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

fn device_id(dev: &IMMDevice) -> anyhow::Result<String> {
    unsafe {
        let p: PWSTR = dev.GetId()?;
        let s = p.to_string().unwrap_or_default();
        CoTaskMemFree(Some(p.0 as *const _));
        Ok(s)
    }
}

fn device_name(dev: &IMMDevice) -> String {
    unsafe {
        let Ok(store) = dev.OpenPropertyStore(STGM_READ) else {
            return String::new();
        };
        let Ok(value) = store.GetValue(&PKEY_Device_FriendlyName) else {
            return String::new();
        };
        match PropVariantToStringAlloc(&value) {
            Ok(p) => {
                let s = p.to_string().unwrap_or_default();
                CoTaskMemFree(Some(p.0 as *const _));
                s
            }
            Err(_) => String::new(),
        }
    }
}

/// Active devices of a flow, with the current default (console role) marked.
pub fn list_devices(flow: AudioFlow) -> anyhow::Result<Vec<AudioDevice>> {
    let en = enumerator()?;
    let f = flow_of(flow);
    let default_id = unsafe { en.GetDefaultAudioEndpoint(f, eConsole) }
        .ok()
        .and_then(|d| device_id(&d).ok());
    let mut out = Vec::new();
    unsafe {
        let coll = en.EnumAudioEndpoints(f, DEVICE_STATE_ACTIVE)?;
        for i in 0..coll.GetCount()? {
            let dev = coll.Item(i)?;
            let id = device_id(&dev)?;
            out.push(AudioDevice {
                name: device_name(&dev),
                is_default: default_id.as_deref() == Some(id.as_str()),
                id,
            });
        }
    }
    Ok(out)
}

/// Makes a device the default for all three roles (console, multimedia, communications).
pub fn set_default_device(id: &str) -> anyhow::Result<()> {
    com_init();
    policy::set_default_endpoint(id)
}

/// Switches to the next active device after the current default.
pub fn cycle_device(flow: AudioFlow) -> anyhow::Result<AudioDevice> {
    let devices = list_devices(flow)?;
    anyhow::ensure!(!devices.is_empty(), "no active audio devices");
    let cur = devices.iter().position(|d| d.is_default).unwrap_or(0);
    let next = devices[(cur + 1) % devices.len()].clone();
    set_default_device(&next.id)?;
    Ok(next)
}
