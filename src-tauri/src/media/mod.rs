//! Media control and "now playing" information (implemented fully in phase 3).

use crate::state::DynamicVisual;

/// Play/pause through the virtual media key.
pub fn play_pause() -> anyhow::Result<()> {
    crate::input::send::tap_vk(0xB3)
}

pub fn now_playing_visual() -> Option<DynamicVisual> {
    None
}
