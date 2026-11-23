//! Deckless: a Stream Deck style action panel that pops up at the mouse cursor.

pub mod app;
pub mod branding;
pub mod editor_window;
pub mod fsutil;
pub mod lang;
pub mod logging;
pub mod paths;
pub mod tray;

use tauri::{Manager, RunEvent};

pub fn run() {
    let _log_guard = logging::init();
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting {}", branding::APP_NAME);

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
        .setup(move |app| {
            let lang = app::initial_lang();
            app.manage(app::AppState::new(lang));
            let tray = tray::create(app.handle(), lang, false)?;
            app.manage(tray);
            if !autostarted {
                editor_window::open(app.handle(), None);
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app, event| {
        // Closing the editor must not quit the app; only the tray "Quit" does (exit code set).
        if let RunEvent::ExitRequested { api, code, .. } = event {
            if code.is_none() {
                api.prevent_exit();
            }
        }
    });
}
