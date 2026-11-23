//! The default profile created on first run (SPEC 5.1).

use super::model::{
    Action, Button, Layout, MediaOp, Profile, SystemOp, VolumeOp, DEFAULT_PROFILE_ID,
};
use crate::keys::KeyCombo;
use crate::lang::Lang;

fn t(lang: Lang, cs: &str, en: &str) -> String {
    match lang {
        Lang::Cs => cs.to_string(),
        Lang::En => en.to_string(),
    }
}

/// Eight useful buttons, clockwise from the top of the radial panel.
pub fn default_profile(lang: Lang) -> Profile {
    let mut p = Profile::new(
        DEFAULT_PROFILE_ID,
        &t(lang, "Výchozí", "Default"),
        Layout::Radial,
    );
    let buttons = vec![
        Button::new(
            &t(lang, "Přehrát/pauza", "Play/Pause"),
            "play",
            Action::Media {
                op: MediaOp::PlayPause,
            },
        ),
        Button::new(
            &t(lang, "Další skladba", "Next track"),
            "skip-forward",
            Action::Media { op: MediaOp::Next },
        ),
        Button::new(
            &t(lang, "Hlasitost +", "Volume +"),
            "volume-2",
            Action::Volume {
                op: VolumeOp::Up,
                step: 5,
                value: 0,
            },
        ),
        Button::new(
            &t(lang, "Hlasitost −", "Volume −"),
            "volume-1",
            Action::Volume {
                op: VolumeOp::Down,
                step: 5,
                value: 0,
            },
        ),
        Button::new(
            &t(lang, "Mikrofon", "Microphone"),
            "mic",
            Action::MicMute {},
        )
        .with_state_b(&t(lang, "Ztlumeno", "Muted"), "mic-off"),
        Button::new(
            &t(lang, "Správce úloh", "Task Manager"),
            "activity",
            Action::hotkey(KeyCombo::new(true, true, false, false, "Escape")),
        ),
        Button::new(
            &t(lang, "Průzkumník", "File Explorer"),
            "folder-open",
            Action::Launch {
                path: "explorer.exe".into(),
                args: String::new(),
                working_dir: String::new(),
                admin: false,
            },
        ),
        Button::new(
            &t(lang, "Výstřižek", "Snip"),
            "scissors",
            Action::System { op: SystemOp::Snip },
        ),
    ];
    for (i, b) in buttons.into_iter().enumerate() {
        p.page.slots[i] = Some(b);
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_fills_all_radial_slots() {
        let p = default_profile(Lang::Cs);
        assert!(p.is_default());
        assert_eq!(p.page.slots.len(), 8);
        assert!(p.page.slots.iter().all(Option::is_some));
    }
}
