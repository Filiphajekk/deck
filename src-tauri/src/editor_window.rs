//! The editor / settings window. Created on demand and destroyed on close to keep idle memory low.

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::branding::APP_NAME;

pub const EDITOR_LABEL: &str = "editor";

/// Shows the editor, creating it if needed. `tab` optionally selects a tab ("settings", "plugins").
pub fn open(app: &AppHandle, tab: Option<&str>) {
    if let Some(win) = app.get_webview_window(EDITOR_LABEL) {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
        if let Some(tab) = tab {
            use tauri::Emitter;
            let _ = win.emit("editor:select-tab", tab);
        }
        return;
    }
    let url = match tab {
        Some(tab) => format!("index.html#{tab}"),
        None => "index.html".to_string(),
    };
    let result = WebviewWindowBuilder::new(app, EDITOR_LABEL, WebviewUrl::App(url.into()))
        .title(APP_NAME)
        .inner_size(1280.0, 800.0)
        .min_inner_size(960.0, 600.0)
        .center()
        // Native file drop is needed to install plugins by dragging; in-app dragging
        // therefore uses pointer events instead of HTML5 drag & drop.
        .build();
    match result {
        Ok(win) => {
            let _ = win.set_focus();
        }
        Err(e) => tracing::error!("failed to open editor window: {e}"),
    }
}
