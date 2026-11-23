//! Stream Deck plugin host (implemented in phase 4).

use crate::profiles::model::Button;
use crate::state::DynamicVisual;

pub fn key_press(_button: &Button) -> anyhow::Result<()> {
    anyhow::bail!("plugins are not supported yet")
}

pub fn button_state(_context: &str) -> u8 {
    0
}

pub fn button_visual(_context: &str) -> Option<DynamicVisual> {
    None
}

/// Called when the active profile or page changes (willAppear / willDisappear).
pub fn on_active_profile_changed() {}
