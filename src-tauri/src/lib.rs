//! Deckless: a Stream Deck style action panel that pops up at the mouse cursor.

pub mod actions;
pub mod app;
pub mod apps;
pub mod audio;
pub mod branding;
pub mod commands;
pub mod config;
pub mod editor_window;
pub mod elevation;
pub mod foreground;
pub mod fsutil;
pub mod icons;
pub mod input;
pub mod keys;
pub mod lang;
pub mod logging;
pub mod media;
pub mod obs;
pub mod overlay;
pub mod paths;
pub mod profiles;
pub mod sdk_host;
pub mod state;
pub mod store;
pub mod tray;
pub mod windows_ops;

use std::sync::atomic::Ordering;

use tauri::{Manager, RunEvent};

/// Relaunches elevated at startup when "Run as administrator" is on (UAC prompt; if the
/// user declines we simply continue unelevated). Returns true if this process should exit.
fn elevate_if_configured() -> bool {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--wait-pid") || elevation::is_elevated() {
        return false;
    }
    if !config::load(&paths::config_file()).run_as_admin {
        return false;
    }
    let extra: Vec<&str> = args
        .iter()
        .skip(1)
        .filter(|a| a.as_str() == "--autostart")
        .map(String::as_str)
        .collect();
    match elevation::relaunch_elevated(&extra) {
        Ok(true) => true,
        Ok(false) => {
            tracing::info!("elevation declined; running without administrator rights");
            false
        }
        Err(e) => {
            tracing::warn!("cannot relaunch elevated: {e:#}");
            false
        }
    }
}

pub fn run() {
    let _log_guard = logging::init();
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        elevated = elevation::is_elevated(),
        "starting {}",
        branding::APP_NAME
    );
    elevation::wait_for_previous_instance();
    if elevate_if_configured() {
        return;
    }

    let autostarted = std::env::args().any(|a| a == "--autostart");
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            tracing::info!(?args, "second instance started");
            if !install_plugins_from_args(&args) {
                editor_window::open(app, None);
            }
        }))
        .register_uri_scheme_protocol(sdk_host::protocol::SCHEME, |_ctx, request| {
            sdk_host::protocol::handle(&request)
        })
        .on_window_event(|window, event| {
            // Plugins dropped onto the editor window are installed (SPEC 7.1).
            if let tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) = event {
                if window.label() == editor_window::EDITOR_LABEL {
                    let paths: Vec<String> = paths
                        .iter()
                        .map(|p| p.to_string_lossy().to_string())
                        .collect();
                    let app = window.app_handle().clone();
                    std::thread::spawn(move || install_dropped(&app, paths));
                }
            }
        })
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            commands::overlay_rendered,
            commands::get_config,
            commands::set_config,
            commands::list_profiles,
            commands::save_profile,
            commands::delete_profile,
            commands::panel_geometry,
            commands::test_button,
            commands::start_hotkey_recording,
            commands::cancel_hotkey_recording,
            commands::latency_stats,
            commands::open_logs_folder,
            commands::is_paused,
            commands::system_language,
            commands::installed_apps,
            commands::running_apps,
            commands::file_icon,
            commands::load_image,
            commands::audio_devices,
            commands::audio_apps,
            commands::export_profile,
            commands::import_profile,
            commands::export_all,
            commands::import_all,
            commands::is_elevated,
            commands::restart_elevated,
            commands::active_profile,
            commands::obs_test,
            commands::obs_lists,
            commands::starter_suggestions,
            commands::list_plugins,
            commands::install_plugin,
            commands::uninstall_plugin,
            commands::restart_plugin,
            commands::plugin_action_defaults,
            commands::property_inspector_url,
            commands::node_available,
            commands::open_url,
            commands::store_catalog,
            commands::store_install,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            let (ctl_tx, ctl_rx) = crossbeam_channel::unbounded();
            let core = app::init(handle.clone(), ctl_tx);

            let (_overlay, hwnd) = overlay::window::create(&handle)?;
            core.overlay_hwnd.store(hwnd, Ordering::SeqCst);

            let input_rx = input::hook::start();
            overlay::controller::start(input_rx, ctl_rx);
            start_foreground_tracking();
            if core.config.read().obs.enabled && obs::profiles_use_obs() {
                obs::ensure_started();
            }
            if media::profiles_use_now_playing() {
                media::ensure_poller();
            }

            if let Err(e) = sdk_host::start_global(handle.clone()) {
                tracing::error!("plugin host failed to start: {e:#}");
            }
            let args: Vec<String> = std::env::args().collect();
            install_plugins_from_args(&args);

            let tray = tray::create(&handle, core.lang(), false)?;
            app.manage(tray);
            app::sync_autostart(&handle);

            if !autostarted {
                editor_window::open(&handle, None);
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app, event| match event {
        // Closing the editor must not quit the app; only the tray "Quit" does (exit code set).
        RunEvent::ExitRequested { api, code, .. } => {
            if code.is_none() {
                api.prevent_exit();
            }
        }
        RunEvent::Exit => {
            input::hook::stop();
            if let Some(h) = sdk_host::host() {
                h.stop_all();
            }
            tracing::info!("exiting");
        }
        _ => {}
    });
}

/// Keeps `last_foreground` and the active profile up to date.
fn start_foreground_tracking() {
    let rx = foreground::watcher::start();
    std::thread::Builder::new()
        .name("foreground-consumer".into())
        .spawn(move || {
            if let Some(f) = foreground::current().filter(|f| !f.is_own_process()) {
                *app::core().last_foreground.lock() = Some(f);
            }
            profiles::active::refresh_active_profile();
            for info in rx {
                if info.is_own_process() {
                    continue;
                }
                let core = app::core();
                let exe_changed = {
                    let mut last = core.last_foreground.lock();
                    let changed = last.as_ref().map(|l| &l.exe) != Some(&info.exe);
                    *last = Some(info);
                    changed
                };
                if exe_changed {
                    *core.manual_profile.lock() = None;
                    profiles::active::refresh_active_profile();
                }
            }
        })
        .expect("spawn foreground consumer");
}

fn is_plugin_file(p: &str) -> bool {
    let l = p.to_lowercase();
    l.ends_with(".streamdeckplugin") || l.ends_with(".sdplugin")
}

/// Installs `.streamDeckPlugin` files passed on the command line (file association).
/// Returns true when there was at least one.
fn install_plugins_from_args(args: &[String]) -> bool {
    let files: Vec<String> = args
        .iter()
        .skip(1)
        .filter(|a| is_plugin_file(a))
        .cloned()
        .collect();
    if files.is_empty() {
        return false;
    }
    let Some(core) = app::try_core() else {
        return false;
    };
    let app = core.app.clone();
    std::thread::spawn(move || install_dropped(&app, files));
    true
}

fn install_dropped(app: &tauri::AppHandle, paths: Vec<String>) {
    use tauri::Emitter;
    let Some(host) = sdk_host::host() else { return };
    for p in paths.into_iter().filter(|p| is_plugin_file(p)) {
        let result = host.install(std::path::Path::new(&p));
        let payload = match &result {
            Ok(info) => serde_json::json!({ "ok": true, "name": info.name }),
            Err(e) => serde_json::json!({ "ok": false, "error": format!("{e:#}"), "file": p }),
        };
        if let Err(e) = &result {
            tracing::error!("plugin install from {p} failed: {e:#}");
        }
        let _ = app.emit("plugins:installed", payload);
        editor_window::open(app, Some("plugins"));
    }
}
