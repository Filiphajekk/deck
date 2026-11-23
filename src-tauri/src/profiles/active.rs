//! Which profile is active right now (follows the foreground app, or a manual switch).

use parking_lot::Mutex;
use tauri::Emitter;

use crate::app::core;

static ACTIVE: Mutex<Option<String>> = Mutex::new(None);

pub fn active_profile_id() -> String {
    ACTIVE
        .lock()
        .clone()
        .unwrap_or_else(|| crate::profiles::model::DEFAULT_PROFILE_ID.to_string())
}

/// Recomputes the active profile from the last foreground app and the manual override.
/// Returns true when it changed.
pub fn refresh_active_profile() -> bool {
    let core = core();
    let exe = core
        .last_foreground
        .lock()
        .as_ref()
        .and_then(|f| f.exe.clone());
    let id = core.profile_id_for(exe.as_deref());
    let changed = {
        let mut cur = ACTIVE.lock();
        if cur.as_deref() == Some(id.as_str()) {
            false
        } else {
            *cur = Some(id.clone());
            true
        }
    };
    if changed {
        tracing::debug!(profile = %id, "active profile changed");
        let _ = core.app.emit("profile:active", &id);
        crate::sdk_host::on_active_profile_changed();
    }
    changed
}
