//! Global application state shared between Tauri commands, the input pipeline and the tray.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Emitter, Manager};

use crate::lang::{self, Lang};

pub struct AppState {
    pub paused: AtomicBool,
    pub lang: parking_lot::RwLock<Lang>,
}

impl AppState {
    pub fn new(lang: Lang) -> Self {
        Self {
            paused: AtomicBool::new(false),
            lang: parking_lot::RwLock::new(lang),
        }
    }

    pub fn lang(&self) -> Lang {
        *self.lang.read()
    }
}

pub fn toggle_paused(app: &AppHandle) {
    let state = app.state::<AppState>();
    let paused = !state.paused.fetch_xor(true, Ordering::SeqCst);
    tracing::info!(paused, "pause toggled");
    crate::tray::refresh(app, state.lang(), paused);
    let _ = app.emit("app:paused", paused);
}

pub fn initial_lang() -> Lang {
    lang::system_lang()
}
