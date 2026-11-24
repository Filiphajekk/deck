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
pub async fn list_profiles() -> Vec<Profile> {
    core().profiles.read().list().to_vec()
}

#[tauri::command]
pub async fn save_profile(profile: Profile) -> CmdResult<()> {
    let core = core();
    core.profiles.write().save(profile).map_err(err)?;
    core.notify_profiles_changed();
    crate::profiles::active::refresh_active_profile();
    crate::sdk_host::refresh_visible();
    Ok(())
}

#[tauri::command]
pub async fn delete_profile(id: String) -> CmdResult<()> {
    let core = core();
    core.profiles.write().delete(&id).map_err(err)?;
    core.notify_profiles_changed();
    crate::profiles::active::refresh_active_profile();
    crate::sdk_host::refresh_visible();
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
pub async fn open_logs_folder() -> CmdResult<()> {
    let dir = crate::paths::logs_dir().to_string_lossy().to_string();
    tauri::async_runtime::spawn_blocking(move || crate::actions::shell::open(&dir))
        .await
        .map_err(err)?
        .map_err(err)
}

#[tauri::command]
pub fn is_paused() -> bool {
    core().is_paused()
}

#[tauri::command]
pub fn system_language() -> &'static str {
    crate::lang::system_lang().code()
}

// ---------------------------------------------------------------------------------------
// Editor helpers: apps, icons, images, audio lists.

#[tauri::command]
pub async fn installed_apps() -> Vec<crate::apps::AppInfo> {
    tauri::async_runtime::spawn_blocking(crate::apps::installed_apps)
        .await
        .unwrap_or_default()
}

#[tauri::command]
pub async fn running_apps() -> Vec<crate::apps::AppInfo> {
    tauri::async_runtime::spawn_blocking(crate::apps::running_apps)
        .await
        .unwrap_or_default()
}

/// Icon of an exe / shortcut / file as a PNG data URL (cached).
#[tauri::command]
pub async fn file_icon(path: String) -> CmdResult<String> {
    tauri::async_runtime::spawn_blocking(move || crate::icons::file_icon_data_url(&path))
        .await
        .map_err(err)?
        .map_err(|e| format!("{e:#}"))
}

/// A user image scaled to fit 144×144, as a data URL.
#[tauri::command]
pub async fn load_image(path: String) -> CmdResult<String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::icons::load_image_data_url(std::path::Path::new(&path))
    })
    .await
    .map_err(err)?
    .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub async fn audio_devices(
    flow: crate::profiles::model::AudioFlow,
) -> CmdResult<Vec<crate::audio::AudioDevice>> {
    tauri::async_runtime::spawn_blocking(move || crate::audio::list_devices(flow))
        .await
        .map_err(err)?
        .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub async fn audio_apps() -> Vec<String> {
    tauri::async_runtime::spawn_blocking(|| {
        crate::audio::com_init();
        crate::audio::sessions::apps_with_sessions()
    })
    .await
    .unwrap_or_default()
}

// ---------------------------------------------------------------------------------------
// Import / export (SPEC 5, 8).

#[tauri::command]
pub async fn export_profile(id: String, path: String) -> CmdResult<()> {
    let profile = core()
        .profiles
        .read()
        .get(&id)
        .cloned()
        .ok_or("profile not found")?;
    crate::fsutil::write_json_atomic(std::path::Path::new(&path), &profile).map_err(err)
}

/// Imports a `.deckless.json` profile as a new profile (new ids) and returns it.
#[tauri::command]
pub async fn import_profile(path: String) -> CmdResult<Profile> {
    let text = std::fs::read_to_string(&path).map_err(err)?;
    let mut profile = crate::profiles::store::parse_profile(&text).map_err(|e| format!("{e:#}"))?;
    profile.id = crate::profiles::model::new_id();
    crate::profiles::store::regenerate_ids(&mut profile.page);
    let core = core();
    core.profiles.write().save(profile.clone()).map_err(err)?;
    core.notify_profiles_changed();
    Ok(profile)
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Backup {
    version: u32,
    kind: String,
    config: Config,
    profiles: Vec<serde_json::Value>,
}

const BACKUP_KIND: &str = "deckless-backup";

#[tauri::command]
pub async fn export_all(path: String) -> CmdResult<()> {
    let core = core();
    let backup = Backup {
        version: 1,
        kind: BACKUP_KIND.into(),
        config: core.config.read().clone(),
        profiles: core
            .profiles
            .read()
            .list()
            .iter()
            .filter_map(|p| serde_json::to_value(p).ok())
            .collect(),
    };
    crate::fsutil::write_json_atomic(std::path::Path::new(&path), &backup).map_err(err)
}

/// Restores settings and profiles from a backup (profiles with the same id are replaced).
#[tauri::command]
pub async fn import_all(path: String) -> CmdResult<usize> {
    let text = std::fs::read_to_string(&path).map_err(err)?;
    let backup: Backup = serde_json::from_str(&text).map_err(|e| format!("invalid backup: {e}"))?;
    if backup.kind != BACKUP_KIND {
        return Err("not a Deckless backup".into());
    }
    let mut profiles = Vec::new();
    for v in backup.profiles {
        let p =
            crate::profiles::store::parse_profile(&v.to_string()).map_err(|e| format!("{e:#}"))?;
        profiles.push(p);
    }
    let core = core();
    let count = profiles.len();
    {
        let mut store = core.profiles.write();
        for p in profiles {
            store.save(p).map_err(err)?;
        }
    }
    let mut cfg = backup.config;
    cfg.first_run_done = true;
    core.update_config(cfg).map_err(err)?;
    core.notify_profiles_changed();
    Ok(count)
}

// ---------------------------------------------------------------------------------------
// Elevation.

#[tauri::command]
pub async fn is_elevated() -> bool {
    crate::elevation::is_elevated()
}

/// Restarts the app elevated (UAC prompt). Returns false if the user declined.
#[tauri::command]
pub async fn restart_elevated(app: tauri::AppHandle) -> CmdResult<bool> {
    let ok =
        crate::elevation::relaunch_elevated(&["--open-editor"]).map_err(|e| format!("{e:#}"))?;
    if ok {
        app.exit(0);
    }
    Ok(ok)
}

/// Id of the profile that is active right now (follows the foreground app).
#[tauri::command]
pub fn active_profile() -> String {
    crate::profiles::active::active_profile_id()
}

// ---------------------------------------------------------------------------------------
// OBS and first-run wizard.

#[tauri::command]
pub async fn obs_test(obs: crate::config::ObsConfig) -> CmdResult<String> {
    crate::obs::test(&obs).await.map_err(|e| format!("{e:#}"))
}

#[derive(Serialize)]
pub struct ObsLists {
    scenes: Vec<String>,
    inputs: Vec<String>,
}

#[tauri::command]
pub async fn obs_lists() -> CmdResult<ObsLists> {
    let (scenes, inputs) = crate::obs::lists().await.map_err(|e| format!("{e:#}"))?;
    Ok(ObsLists { scenes, inputs })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StarterSuggestions {
    profiles: Vec<Profile>,
    plugin_apps: Vec<crate::profiles::starters::PluginSuggestion>,
}

/// Starter profiles for apps found on this PC (not saved yet) and plugin suggestions.
#[tauri::command]
pub async fn starter_suggestions() -> StarterSuggestions {
    let installed: std::collections::HashSet<String> = tauri::async_runtime::spawn_blocking(|| {
        let mut s: std::collections::HashSet<String> = crate::apps::installed_apps()
            .into_iter()
            .map(|a| a.exe)
            .collect();
        s.extend(crate::apps::running_apps().into_iter().map(|a| a.exe));
        s
    })
    .await
    .unwrap_or_default();
    let core = core();
    let obs_cfg = core.config.read().obs.clone();
    let scenes = if obs_cfg.enabled {
        crate::obs::lists().await.map(|l| l.0).ok()
    } else {
        None
    };
    let existing: std::collections::HashSet<String> = core
        .profiles
        .read()
        .list()
        .iter()
        .flat_map(|p| p.apps.clone())
        .collect();
    let profiles = crate::profiles::starters::starter_profiles(
        core.lang(),
        &installed,
        scenes.is_some(),
        scenes.as_deref().unwrap_or(&[]),
    )
    .into_iter()
    // Skip apps that already have a profile.
    .filter(|p| !p.apps.iter().any(|a| existing.contains(a)))
    .collect();
    StarterSuggestions {
        profiles,
        plugin_apps: crate::profiles::starters::plugin_suggestions(&installed),
    }
}

// ---------------------------------------------------------------------------------------
// Stream Deck plugins (SPEC 7, 8).

fn plugin_host() -> CmdResult<&'static std::sync::Arc<crate::sdk_host::Host>> {
    crate::sdk_host::host().ok_or_else(|| "the plugin host is not running".to_string())
}

/// Runs blocking work off the main (UI) thread. Sync Tauri commands run on the main
/// thread, so anything touching files, images, processes or the plugin host goes here;
/// otherwise the editor and the overlay freeze (Windows reports the app as hung).
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> CmdResult<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(err)
}

#[tauri::command]
pub async fn list_plugins() -> CmdResult<Vec<crate::sdk_host::PluginInfo>> {
    blocking(|| {
        crate::sdk_host::host()
            .map(|h| h.list())
            .unwrap_or_default()
    })
    .await
}

#[tauri::command]
pub async fn install_plugin(path: String) -> CmdResult<crate::sdk_host::PluginInfo> {
    let host = plugin_host()?.clone();
    blocking(move || host.install(std::path::Path::new(&path)))
        .await?
        .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub async fn uninstall_plugin(uuid: String) -> CmdResult<()> {
    let host = plugin_host()?.clone();
    blocking(move || host.uninstall(&uuid))
        .await?
        .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub async fn restart_plugin(uuid: String) -> CmdResult<()> {
    let host = plugin_host()?.clone();
    blocking(move || {
        host.stop_plugin(&uuid);
        host.start_plugin(&uuid);
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginButtonDefaults {
    title: String,
    icon: Option<crate::profiles::model::Icon>,
    state_b: Option<crate::profiles::model::Visual>,
}

#[tauri::command]
pub async fn plugin_action_defaults(
    plugin_uuid: String,
    action_uuid: String,
) -> CmdResult<PluginButtonDefaults> {
    let host = plugin_host()?.clone();
    let (title, icon, state_b) = blocking(move || host.action_defaults(&plugin_uuid, &action_uuid))
        .await?
        .ok_or("unknown plugin action")?;
    Ok(PluginButtonDefaults {
        title,
        icon,
        state_b,
    })
}

#[tauri::command]
pub async fn property_inspector_url(
    plugin_uuid: String,
    action_uuid: String,
    context: String,
) -> CmdResult<Option<String>> {
    let host = plugin_host()?.clone();
    blocking(move || host.property_inspector_url(&plugin_uuid, &action_uuid, &context))
        .await?
        .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub async fn node_available() -> CmdResult<bool> {
    blocking(|| crate::sdk_host::find_node().is_some()).await
}

#[tauri::command]
pub async fn open_url(url: String) -> CmdResult<()> {
    let lower = url.to_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) {
        return Err("only web addresses can be opened".into());
    }
    blocking(move || crate::actions::shell::open(&url))
        .await?
        .map_err(err)
}

// ---------------------------------------------------------------------------------------
// Plugin store.

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreItem {
    #[serde(flatten)]
    entry: crate::store::StoreEntry,
    installed: bool,
    installed_version: Option<String>,
}

#[tauri::command]
pub async fn store_catalog() -> CmdResult<Vec<StoreItem>> {
    blocking(|| {
        // Only versions are needed here; no icons are decoded.
        let installed = crate::sdk_host::host()
            .map(|h| h.versions())
            .unwrap_or_default();
        crate::store::catalog()
            .into_iter()
            .map(|entry| StoreItem {
                installed: installed.contains_key(&entry.uuid),
                installed_version: installed.get(&entry.uuid).cloned(),
                entry,
            })
            .collect()
    })
    .await
}

/// Downloads the plugin from its author's GitHub releases and installs it.
#[tauri::command]
pub async fn store_install(id: String) -> CmdResult<crate::sdk_host::PluginInfo> {
    tauri::async_runtime::spawn_blocking(move || crate::store::install(&id))
        .await
        .map_err(err)?
        .map_err(|e| format!("{e:#}"))
}
