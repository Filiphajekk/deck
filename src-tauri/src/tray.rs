//! Tray icon with "Open editor", "Pause" and "Quit" (SPEC 3.2).

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::lang::{tr, Lang, Text};

pub const TRAY_ID: &str = "main";

pub struct TrayHandles {
    pub icon: TrayIcon<Wry>,
    pub pause_item: MenuItem<Wry>,
}

pub fn create(app: &AppHandle, lang: Lang, paused: bool) -> tauri::Result<TrayHandles> {
    let open = MenuItem::with_id(
        app,
        "open",
        tr(lang, Text::TrayOpenEditor),
        true,
        None::<&str>,
    )?;
    let pause = MenuItem::with_id(app, "pause", pause_text(lang, paused), true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", tr(lang, Text::TrayQuit), true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &pause, &sep, &quit])?;

    let icon = TrayIconBuilder::with_id(TRAY_ID)
        .icon(
            app.default_window_icon()
                .cloned()
                .expect("app icon is bundled"),
        )
        .tooltip(tooltip(lang, paused))
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => crate::editor_window::open(app, None),
            "pause" => crate::app::toggle_paused(app),
            "quit" => {
                tracing::info!("quit requested from tray");
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                crate::editor_window::open(tray.app_handle(), None);
            }
        })
        .build(app)?;

    Ok(TrayHandles {
        icon,
        pause_item: pause,
    })
}

fn pause_text(lang: Lang, paused: bool) -> &'static str {
    if paused {
        tr(lang, Text::TrayResume)
    } else {
        tr(lang, Text::TrayPause)
    }
}

fn tooltip(lang: Lang, paused: bool) -> &'static str {
    if paused {
        tr(lang, Text::TrayTooltipPaused)
    } else {
        tr(lang, Text::TrayTooltip)
    }
}

/// Updates texts after a pause toggle or a language change.
pub fn refresh(app: &AppHandle, lang: Lang, paused: bool) {
    if let Some(handles) = app.try_state::<TrayHandles>() {
        let _ = handles.pause_item.set_text(pause_text(lang, paused));
        let _ = handles.icon.set_tooltip(Some(tooltip(lang, paused)));
    }
}
