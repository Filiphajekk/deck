//! Live button state (muted mic, toggle B, OBS recording...) and dynamic visuals
//! (now playing, plugin titles/images).

use crate::app::core;
use crate::profiles::model::{Action, Button, Icon, ObsOp, VolumeOp};

/// 0 = first state, 1 = second state.
pub fn button_state(b: &Button) -> u8 {
    match &b.action {
        Action::MicMute {} => crate::audio::mic_muted().unwrap_or(false) as u8,
        Action::Volume {
            op: VolumeOp::MuteToggle,
            ..
        } => crate::audio::output_muted().unwrap_or(false) as u8,
        Action::AppVolume {
            op: VolumeOp::MuteToggle,
            exe,
            ..
        } => crate::audio::sessions::app_muted(exe) as u8,
        Action::Toggle { .. } => core().toggles.lock().get(&b.id).copied().unwrap_or(0),
        Action::Obs { op, source, .. } => {
            let s = crate::obs::status();
            match op {
                ObsOp::ToggleRecord | ObsOp::StartRecord | ObsOp::StopRecord => s.recording as u8,
                ObsOp::ToggleStream | ObsOp::StartStream | ObsOp::StopStream => s.streaming as u8,
                ObsOp::ToggleMute => s.muted_sources.contains(source) as u8,
                ObsOp::SwitchScene | ObsOp::SaveReplay => 0,
            }
        }
        Action::Plugin { .. } => crate::sdk_host::button_state(&b.id),
        _ => 0,
    }
}

#[derive(Debug, Default)]
pub struct DynamicVisual {
    pub title: Option<String>,
    pub icon: Option<Icon>,
}

/// Title/icon that come from outside the profile (media session, plugins).
pub fn dynamic_visual(b: &Button) -> Option<DynamicVisual> {
    match &b.action {
        Action::NowPlaying {} => crate::media::now_playing_visual(),
        Action::Plugin { .. } => crate::sdk_host::button_visual(&b.id),
        _ => None,
    }
}
