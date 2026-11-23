//! Language selection for the few strings the backend shows itself (tray menu, hints).
//! The UI texts live in `src/i18n/*.json`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LanguagePref {
    #[default]
    Auto,
    Cs,
    En,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    Cs,
    En,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::Cs => "cs",
            Lang::En => "en",
        }
    }
}

/// Czech when the Windows UI language is Czech or Slovak, otherwise English (SPEC 9).
pub fn system_lang() -> Lang {
    // LANGID: low 10 bits are the primary language. 0x05 = Czech, 0x1B = Slovak.
    let langid = unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() };
    lang_from_langid(langid)
}

pub fn lang_from_langid(langid: u16) -> Lang {
    match langid & 0x3ff {
        0x05 | 0x1b => Lang::Cs,
        _ => Lang::En,
    }
}

pub fn resolve(pref: LanguagePref) -> Lang {
    match pref {
        LanguagePref::Auto => system_lang(),
        LanguagePref::Cs => Lang::Cs,
        LanguagePref::En => Lang::En,
    }
}

/// Backend strings. Keys are matched exhaustively so nothing is left untranslated.
#[derive(Debug, Clone, Copy)]
pub enum Text {
    TrayOpenEditor,
    TrayPause,
    TrayResume,
    TrayQuit,
    TrayTooltip,
    TrayTooltipPaused,
}

pub fn tr(lang: Lang, text: Text) -> &'static str {
    use Text::*;
    match (lang, text) {
        (Lang::Cs, TrayOpenEditor) => "Otevřít editor",
        (Lang::En, TrayOpenEditor) => "Open editor",
        (Lang::Cs, TrayPause) => "Pozastavit",
        (Lang::En, TrayPause) => "Pause",
        (Lang::Cs, TrayResume) => "Pokračovat",
        (Lang::En, TrayResume) => "Resume",
        (Lang::Cs, TrayQuit) => "Ukončit",
        (Lang::En, TrayQuit) => "Quit",
        (Lang::Cs, TrayTooltip) => "Deckless",
        (Lang::En, TrayTooltip) => "Deckless",
        (Lang::Cs, TrayTooltipPaused) => "Deckless (pozastaveno)",
        (Lang::En, TrayTooltipPaused) => "Deckless (paused)",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn czech_and_slovak_map_to_cs() {
        assert_eq!(lang_from_langid(0x0405), Lang::Cs);
        assert_eq!(lang_from_langid(0x041b), Lang::Cs);
        assert_eq!(lang_from_langid(0x0409), Lang::En);
        assert_eq!(lang_from_langid(0x0407), Lang::En);
    }
}
