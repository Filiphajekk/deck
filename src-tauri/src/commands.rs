//! Tauri commands called from the editor and the overlay. Errors are returned as strings.

use serde::Serialize;

use crate::app::core;
use crate::config::Config;
use crate::input::hook::SHARED;
use crate::overlay::controller::Msg;
use crate::overlay::geometry::{self, PanelGeometry};
use crate::profiles::model::{Action, Button, Layout, Profile};

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[tauri::command]
pub fn overlay_rendered(seq: u64) {
    let _ = core().controller.send(Msg::Rendered { seq });
}

#[tauri::command]
pub fn get_config() -> Config {
    core().config.read().clone()
}

#[tauri::command]
pub async fn set_config(config: Config) -> CmdResult<()> {
    core().update_config(config).map_err(err)
}

#[tauri::command]
pub fn list_profiles() -> Vec<Profile> {
    core().profiles.read().list().to_vec()
}

#[tauri::command]
pub async fn save_profile(profile: Profile) -> CmdResult<()> {
    let core = core();
    core.profiles.write().save(profile).map_err(err)?;
    core.notify_profiles_changed();
    crate::profiles::active::refresh_active_profile();
    Ok(())
}

#[tauri::command]
pub async fn delete_profile(id: String) -> CmdResult<()> {
    let core = core();
    core.profiles.write().delete(&id).map_err(err)?;
    core.notify_profiles_changed();
    crate::profiles::active::refresh_active_profile();
    Ok(())
}

#[tauri::command]
pub fn panel_geometry(layout: Layout) -> PanelGeometry {
    geometry::panel_geometry(layout, core().config.read().appearance.size)
}

/// "Try it" in the editor: runs the button's action right away. Keystrokes go to the
/// window that was in the foreground before the editor.
#[tauri::command]
pub async fn test_button(button: Button) -> CmdResult<()> {
    let core = core();
    let target = core.last_foreground.lock().clone();
    let needs_target = matches!(
        button.action,
        Action::Hotkey { .. } | Action::TypeText { .. } | Action::Window { .. }
    );
    tauri::async_runtime::spawn_blocking(move || {
        crate::audio::com_init();
        let ctx = crate::actions::ActionCtx {
            target: if needs_target { target } else { None },
            button_id: Some(button.id.clone()),
            from_panel: needs_target,
        };
        crate::actions::run_button_action(&button, &ctx)
    })
    .await
    .map_err(err)?
    .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub fn start_hotkey_recording() {
    SHARED.start_recording();
}

#[tauri::command]
pub fn cancel_hotkey_recording() {
    SHARED.cancel_recording();
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LatencyStats {
    pub count: usize,
    pub avg_ms: f64,
    pub max_ms: f64,
    pub last_ms: f64,
}

#[tauri::command]
pub fn latency_stats() -> LatencyStats {
    let l = core().latencies.lock();
    let count = l.len();
    let sum: f64 = l.iter().sum();
    LatencyStats {
        count,
        avg_ms: if count > 0 { sum / count as f64 } else { 0.0 },
        max_ms: l.iter().cloned().fold(0.0, f64::max),
        last_ms: l.back().copied().unwrap_or(0.0),
    }
}

#[tauri::command]
pub fn open_logs_folder() -> CmdResult<()> {
    crate::actions::shell::open(&crate::paths::logs_dir().to_string_lossy()).map_err(err)
}

#[tauri::command]
pub fn is_paused() -> bool {
    core().is_paused()
}

#[tauri::command]
pub fn system_language() -> &'static str {
    crate::lang::system_lang().code()
}
