//! Deckless: a Stream Deck style action panel that pops up at the mouse cursor.

pub mod actions;
pub mod app;
pub mod audio;
pub mod branding;
pub mod commands;
pub mod config;
pub mod editor_window;
pub mod foreground;
pub mod fsutil;
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
pub mod tray;
pub mod windows_ops;

use std::sync::atomic::Ordering;

use tauri::{Manager, RunEvent};

pub fn run() {
    let _log_guard = logging::init();
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        "starting {}",
        branding::APP_NAME
    );

    let autostarted = std::env::args().any(|a| a == "--autostart");

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            tracing::info!(?args, "second instance started");
            editor_window::open(app, None);
        }))
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

            let tray = tray::create(&handle, core.lang(), false)?;
            app.manage(tray);
            sync_autostart(&handle);

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

/// Applies the autostart setting to the registry (SPEC 8: on by default).
fn sync_autostart(app: &tauri::AppHandle) {
    use tauri_plugin_autostart::ManagerExt;
    let want = app::core().config.read().autostart;
    // Dev builds must not register themselves to start at login.
    if cfg!(debug_assertions) {
        return;
    }
    let mgr = app.autolaunch();
    let have = mgr.is_enabled().unwrap_or(false);
    let result = match (want, have) {
        (true, false) => mgr.enable(),
        (false, true) => mgr.disable(),
        _ => Ok(()),
    };
    if let Err(e) = result {
        tracing::warn!("cannot update autostart: {e}");
    }
}
