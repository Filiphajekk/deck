//! Ready-made starter profiles for apps found on the system (SPEC 5.1). Only default
//! shortcuts that the apps really use out of the box.

use std::collections::HashSet;

use serde::Serialize;

use super::model::{Action, Button, Layout, ObsOp, Profile};
use crate::keys::KeyCombo;
use crate::lang::Lang;

fn t(lang: Lang, cs: &str, en: &str) -> String {
    match lang {
        Lang::Cs => cs.into(),
        Lang::En => en.into(),
    }
}

fn hk(ctrl: bool, shift: bool, alt: bool, key: &str) -> Action {
    Action::hotkey(KeyCombo::new(ctrl, shift, alt, false, key))
}

fn profile(id: &str, name: &str, apps: Vec<String>, buttons: Vec<Button>) -> Profile {
    let mut p = Profile::new(id, name, Layout::Radial);
    p.apps = apps;
    for (i, b) in buttons.into_iter().enumerate().take(8) {
        p.page.slots[i] = Some(b);
    }
    p
}

pub const CHROMIUM_BROWSERS: [&str; 4] = ["chrome.exe", "msedge.exe", "brave.exe", "opera.exe"];
pub const FIREFOX: &str = "firefox.exe";
pub const VSCODE: &str = "code.exe";
pub const OBS: &str = "obs64.exe";

/// Apps for which we suggest installing a Stream Deck plugin instead of shortcuts.
pub const PLUGIN_APPS: [(&str, &str); 4] = [
    ("discord.exe", "Discord"),
    ("spotify.exe", "Spotify"),
    ("twitch.exe", "Twitch"),
    ("teams.exe", "Microsoft Teams"),
];

fn browser_buttons(lang: Lang, incognito: Action) -> Vec<Button> {
    vec![
        Button::new(
            &t(lang, "Nová karta", "New tab"),
            "plus",
            hk(true, false, false, "T"),
        ),
        Button::new(
            &t(lang, "Obnovit kartu", "Reopen tab"),
            "rotate-ccw",
            hk(true, true, false, "T"),
        ),
        Button::new(
            &t(lang, "Zavřít kartu", "Close tab"),
            "x",
            hk(true, false, false, "W"),
        ),
        Button::new(
            &t(lang, "Anonymní okno", "Private window"),
            "eye-off",
            incognito,
        ),
        Button::new(
            &t(lang, "Obnovit", "Reload"),
            "refresh-cw",
            hk(false, false, false, "F5"),
        ),
        Button::new(
            &t(lang, "Historie", "History"),
            "history",
            hk(true, false, false, "H"),
        ),
    ]
}

/// Starter profiles for the apps present on the system. `obs_scenes` is `Some` when the
/// OBS connection works (scene buttons are added then).
pub fn starter_profiles(
    lang: Lang,
    installed: &HashSet<String>,
    obs_configured: bool,
    obs_scenes: &[String],
) -> Vec<Profile> {
    let mut out = Vec::new();
    let chromium: Vec<String> = CHROMIUM_BROWSERS
        .iter()
        .filter(|e| installed.contains(**e))
        .map(|e| e.to_string())
        .collect();
    if !chromium.is_empty() {
        out.push(profile(
            "starter-chromium",
            &t(lang, "Prohlížeč", "Browser"),
            chromium,
            browser_buttons(lang, hk(true, true, false, "N")),
        ));
    }
    if installed.contains(FIREFOX) {
        out.push(profile(
            "starter-firefox",
            "Firefox",
            vec![FIREFOX.into()],
            browser_buttons(lang, hk(true, true, false, "P")),
        ));
    }
    if installed.contains(VSCODE) {
        out.push(profile(
            "starter-vscode",
            "VS Code",
            vec![VSCODE.into()],
            vec![
                Button::new(
                    &t(lang, "Paleta příkazů", "Command palette"),
                    "command",
                    hk(true, true, false, "P"),
                ),
                Button::new(
                    &t(lang, "Terminál", "Terminal"),
                    "square-terminal",
                    hk(true, false, false, "Backquote"),
                ),
                Button::new(
                    &t(lang, "Hledat v souborech", "Find in files"),
                    "search",
                    hk(true, true, false, "F"),
                ),
                Button::new(
                    &t(lang, "Boční panel", "Sidebar"),
                    "panel-left",
                    hk(true, false, false, "B"),
                ),
                Button::new(
                    &t(lang, "Formátovat", "Format document"),
                    "wand-2",
                    hk(false, true, true, "F"),
                ),
            ],
        ));
    }
    if installed.contains(OBS) && obs_configured {
        let obs = |op: ObsOp| Action::Obs {
            op,
            scene: String::new(),
            source: String::new(),
        };
        let mut buttons = vec![
            Button::new(
                &t(lang, "Nahrávání", "Recording"),
                "circle-dot",
                obs(ObsOp::ToggleRecord),
            )
            .with_state_b(&t(lang, "Nahrává", "Recording"), "circle-stop"),
            Button::new(
                &t(lang, "Stream", "Stream"),
                "radio-tower",
                obs(ObsOp::ToggleStream),
            )
            .with_state_b(&t(lang, "Živě", "Live"), "circle-stop"),
            Button::new(
                &t(lang, "Uložit replay", "Save replay"),
                "history",
                obs(ObsOp::SaveReplay),
            ),
        ];
        for scene in obs_scenes.iter().take(5) {
            buttons.push(Button::new(
                scene,
                "clapperboard",
                Action::Obs {
                    op: ObsOp::SwitchScene,
                    scene: scene.clone(),
                    source: String::new(),
                },
            ));
        }
        out.push(profile("starter-obs", "OBS", vec![OBS.into()], buttons));
    }
    out
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSuggestion {
    pub exe: String,
    pub name: String,
}

pub fn plugin_suggestions(installed: &HashSet<String>) -> Vec<PluginSuggestion> {
    PLUGIN_APPS
        .iter()
        .filter(|(exe, _)| installed.contains(*exe))
        .map(|(exe, name)| PluginSuggestion {
            exe: exe.to_string(),
            name: name.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(items: &[&str]) -> HashSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn only_installed_apps_get_profiles() {
        let p = starter_profiles(
            Lang::En,
            &set(&["brave.exe", "msedge.exe", "notepad.exe"]),
            false,
            &[],
        );
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].apps, vec!["msedge.exe", "brave.exe"]);
        // Chromium uses Ctrl+Shift+N for a private window.
        let incognito = p[0].page.slots[3].as_ref().unwrap();
        assert_eq!(incognito.action, hk(true, true, false, "N"));
    }

    #[test]
    fn firefox_uses_ctrl_shift_p() {
        let p = starter_profiles(Lang::Cs, &set(&["firefox.exe"]), false, &[]);
        assert_eq!(
            p[0].page.slots[3].as_ref().unwrap().action,
            hk(true, true, false, "P")
        );
    }

    #[test]
    fn obs_needs_connection() {
        let installed = set(&["obs64.exe", "code.exe"]);
        assert_eq!(starter_profiles(Lang::En, &installed, false, &[]).len(), 1);
        let with = starter_profiles(Lang::En, &installed, true, &["Game".into(), "Chat".into()]);
        let obs = with.iter().find(|p| p.id == "starter-obs").unwrap();
        assert_eq!(obs.page.slots.iter().flatten().count(), 5);
    }

    #[test]
    fn plugin_suggestions_for_found_apps() {
        let s = plugin_suggestions(&set(&["discord.exe", "chrome.exe"]));
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].name, "Discord");
    }
}
