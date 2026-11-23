//! Key names, key combinations and building `SendInput` key sequences.
//!
//! Keys are stored in profiles by readable names ("A", "F5", "Space", "Backquote"...) and
//! mapped to Windows virtual-key codes here. The TS mirror is `src/lib/keys.ts`.

use serde::{Deserialize, Serialize};

/// A key combination such as Ctrl+Shift+T. `key` is `None` only for incomplete combos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KeyCombo {
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub alt: bool,
    #[serde(default)]
    pub win: bool,
    #[serde(default)]
    pub key: Option<String>,
}

impl KeyCombo {
    pub fn new(ctrl: bool, shift: bool, alt: bool, win: bool, key: &str) -> Self {
        Self {
            ctrl,
            shift,
            alt,
            win,
            key: Some(key.to_string()),
        }
    }

    pub fn mods(&self) -> Mods {
        Mods {
            ctrl: self.ctrl,
            shift: self.shift,
            alt: self.alt,
            win: self.win,
        }
    }

    /// Win+L cannot be injected (secure attention), it must go through `LockWorkStation`.
    pub fn is_win_l(&self) -> bool {
        self.win && !self.ctrl && !self.alt && !self.shift && self.key.as_deref() == Some("L")
    }

    pub fn vk(&self) -> Option<u16> {
        self.key.as_deref().and_then(vk_from_name)
    }
}

impl std::fmt::Display for KeyCombo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.win {
            parts.push("Win");
        }
        if let Some(k) = &self.key {
            parts.push(k);
        }
        write!(f, "{}", parts.join("+"))
    }
}

/// Modifier state as a compact value (shared with the hook thread through atomics).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Mods {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub win: bool,
}

impl Mods {
    pub fn bits(self) -> u8 {
        (self.ctrl as u8) | (self.shift as u8) << 1 | (self.alt as u8) << 2 | (self.win as u8) << 3
    }

    pub fn from_bits(b: u8) -> Self {
        Self {
            ctrl: b & 1 != 0,
            shift: b & 2 != 0,
            alt: b & 4 != 0,
            win: b & 8 != 0,
        }
    }

    pub fn any(self) -> bool {
        self.bits() != 0
    }
}

pub const VK_SHIFT: u16 = 0x10;
pub const VK_CONTROL: u16 = 0x11;
pub const VK_MENU: u16 = 0x12;
pub const VK_LSHIFT: u16 = 0xA0;
pub const VK_RSHIFT: u16 = 0xA1;
pub const VK_LCONTROL: u16 = 0xA2;
pub const VK_RCONTROL: u16 = 0xA3;
pub const VK_LMENU: u16 = 0xA4;
pub const VK_RMENU: u16 = 0xA5;
pub const VK_LWIN: u16 = 0x5B;
pub const VK_RWIN: u16 = 0x5C;
pub const VK_RETURN: u16 = 0x0D;
pub const VK_TAB: u16 = 0x09;
pub const VK_ESCAPE: u16 = 0x1B;
pub const VK_BACK: u16 = 0x08;
pub const VK_LEFT: u16 = 0x25;
pub const VK_UP: u16 = 0x26;
pub const VK_RIGHT: u16 = 0x27;
pub const VK_DOWN: u16 = 0x28;
/// Unassigned virtual key used to "mask" a lone Win/Alt release so that the Start menu
/// or the window menu bar does not open after the trigger hotkey was swallowed.
pub const VK_MASK: u16 = 0xE8;

pub fn is_modifier_vk(vk: u16) -> bool {
    matches!(
        vk,
        VK_SHIFT
            | VK_CONTROL
            | VK_MENU
            | VK_LSHIFT
            | VK_RSHIFT
            | VK_LCONTROL
            | VK_RCONTROL
            | VK_LMENU
            | VK_RMENU
            | VK_LWIN
            | VK_RWIN
    )
}

/// (name, virtual-key code, needs KEYEVENTF_EXTENDEDKEY)
const NAMED_KEYS: &[(&str, u16, bool)] = &[
    ("Space", 0x20, false),
    ("Enter", 0x0D, false),
    ("Escape", 0x1B, false),
    ("Tab", 0x09, false),
    ("Backspace", 0x08, false),
    ("Delete", 0x2E, true),
    ("Insert", 0x2D, true),
    ("Home", 0x24, true),
    ("End", 0x23, true),
    ("PageUp", 0x21, true),
    ("PageDown", 0x22, true),
    ("Left", 0x25, true),
    ("Up", 0x26, true),
    ("Right", 0x27, true),
    ("Down", 0x28, true),
    ("PrintScreen", 0x2C, true),
    ("Pause", 0x13, false),
    ("CapsLock", 0x14, false),
    ("NumLock", 0x90, true),
    ("ScrollLock", 0x91, false),
    ("ContextMenu", 0x5D, true),
    ("Semicolon", 0xBA, false),
    ("Equal", 0xBB, false),
    ("Comma", 0xBC, false),
    ("Minus", 0xBD, false),
    ("Period", 0xBE, false),
    ("Slash", 0xBF, false),
    ("Backquote", 0xC0, false),
    ("BracketLeft", 0xDB, false),
    ("Backslash", 0xDC, false),
    ("BracketRight", 0xDD, false),
    ("Quote", 0xDE, false),
    ("IntlBackslash", 0xE2, false),
    ("NumpadMultiply", 0x6A, false),
    ("NumpadAdd", 0x6B, false),
    ("NumpadSubtract", 0x6D, false),
    ("NumpadDecimal", 0x6E, false),
    ("NumpadDivide", 0x6F, true),
    ("MediaNext", 0xB0, true),
    ("MediaPrev", 0xB1, true),
    ("MediaStop", 0xB2, true),
    ("MediaPlayPause", 0xB3, true),
    ("VolumeMute", 0xAD, true),
    ("VolumeDown", 0xAE, true),
    ("VolumeUp", 0xAF, true),
    ("BrowserBack", 0xA6, true),
    ("BrowserForward", 0xA7, true),
    ("BrowserRefresh", 0xA8, true),
    ("BrowserHome", 0xAC, true),
];

pub fn vk_from_name(name: &str) -> Option<u16> {
    let bytes = name.as_bytes();
    if bytes.len() == 1 {
        let c = bytes[0].to_ascii_uppercase();
        if c.is_ascii_uppercase() || c.is_ascii_digit() {
            return Some(c as u16);
        }
    }
    if let Some(n) = name.strip_prefix('F').and_then(|n| n.parse::<u16>().ok()) {
        if (1..=24).contains(&n) {
            return Some(0x70 + n - 1);
        }
    }
    if let Some(n) = name
        .strip_prefix("Numpad")
        .and_then(|n| n.parse::<u16>().ok())
    {
        if n <= 9 {
            return Some(0x60 + n);
        }
    }
    NAMED_KEYS
        .iter()
        .find(|(n, _, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, vk, _)| *vk)
}

pub fn name_from_vk(vk: u16) -> Option<String> {
    match vk {
        0x41..=0x5A | 0x30..=0x39 => Some((vk as u8 as char).to_string()),
        0x70..=0x87 => Some(format!("F{}", vk - 0x70 + 1)),
        0x60..=0x69 => Some(format!("Numpad{}", vk - 0x60)),
        _ => NAMED_KEYS
            .iter()
            .find(|(_, v, _)| *v == vk)
            .map(|(n, _, _)| n.to_string()),
    }
}

pub fn is_extended(vk: u16) -> bool {
    matches!(vk, VK_RCONTROL | VK_RMENU | VK_LWIN | VK_RWIN)
        || NAMED_KEYS.iter().any(|(_, v, ext)| *v == vk && *ext)
}

/// One synthetic key event, independent of the Win32 INPUT struct so it can be unit-tested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEvent {
    /// Virtual key press/release.
    Vk { vk: u16, up: bool },
    /// A UTF-16 code unit sent with KEYEVENTF_UNICODE.
    Unicode { unit: u16, up: bool },
}

fn modifier_vks(m: Mods) -> Vec<u16> {
    let mut v = Vec::new();
    if m.ctrl {
        v.push(VK_LCONTROL);
    }
    if m.shift {
        v.push(VK_LSHIFT);
    }
    if m.alt {
        v.push(VK_LMENU);
    }
    if m.win {
        v.push(VK_LWIN);
    }
    v
}

/// Press modifiers, press+release the key, release modifiers in reverse order.
pub fn combo_events(combo: &KeyCombo) -> Vec<KeyEvent> {
    let mods = modifier_vks(combo.mods());
    let mut out: Vec<KeyEvent> = mods
        .iter()
        .map(|&vk| KeyEvent::Vk { vk, up: false })
        .collect();
    if let Some(vk) = combo.vk() {
        out.push(KeyEvent::Vk { vk, up: false });
        out.push(KeyEvent::Vk { vk, up: true });
    }
    out.extend(mods.iter().rev().map(|&vk| KeyEvent::Vk { vk, up: true }));
    out
}

/// Given the events that were actually delivered (a prefix of `events`), returns the
/// key-up events needed so that no key stays pressed (used when SendInput is cut short).
pub fn cleanup_after_partial(events: &[KeyEvent], delivered: usize) -> Vec<KeyEvent> {
    let mut held: Vec<KeyEvent> = Vec::new();
    for ev in events.iter().take(delivered) {
        match *ev {
            KeyEvent::Vk { vk, up: false } => held.push(KeyEvent::Vk { vk, up: true }),
            KeyEvent::Vk { vk, up: true } => held.retain(|h| *h != KeyEvent::Vk { vk, up: true }),
            KeyEvent::Unicode { unit, up: false } => {
                held.push(KeyEvent::Unicode { unit, up: true })
            }
            KeyEvent::Unicode { unit, up: true } => {
                held.retain(|h| *h != KeyEvent::Unicode { unit, up: true })
            }
        }
    }
    held.reverse();
    held
}

/// Text as Unicode key events. Line breaks become Enter and tabs become Tab, so that
/// they work in editors that ignore Unicode control characters.
pub fn text_events(text: &str) -> Vec<KeyEvent> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    let press = |out: &mut Vec<KeyEvent>, vk: u16| {
        out.push(KeyEvent::Vk { vk, up: false });
        out.push(KeyEvent::Vk { vk, up: true });
    };
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                press(&mut out, VK_RETURN);
            }
            '\n' => press(&mut out, VK_RETURN),
            '\t' => press(&mut out, VK_TAB),
            _ => {
                let mut buf = [0u16; 2];
                for &unit in c.encode_utf16(&mut buf).iter() {
                    out.push(KeyEvent::Unicode { unit, up: false });
                    out.push(KeyEvent::Unicode { unit, up: true });
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_roundtrip() {
        for name in [
            "A",
            "Z",
            "0",
            "9",
            "F1",
            "F24",
            "Space",
            "Backquote",
            "Numpad5",
            "Left",
        ] {
            let vk = vk_from_name(name).unwrap_or_else(|| panic!("{name}"));
            assert_eq!(name_from_vk(vk).as_deref(), Some(name));
        }
        assert_eq!(vk_from_name("a"), Some(0x41));
        assert_eq!(vk_from_name("F25"), None);
        assert_eq!(vk_from_name("Nope"), None);
    }

    #[test]
    fn ctrl_shift_t_sequence() {
        let combo = KeyCombo::new(true, true, false, false, "T");
        let ev = combo_events(&combo);
        assert_eq!(
            ev,
            vec![
                KeyEvent::Vk {
                    vk: VK_LCONTROL,
                    up: false
                },
                KeyEvent::Vk {
                    vk: VK_LSHIFT,
                    up: false
                },
                KeyEvent::Vk {
                    vk: 0x54,
                    up: false
                },
                KeyEvent::Vk { vk: 0x54, up: true },
                KeyEvent::Vk {
                    vk: VK_LSHIFT,
                    up: true
                },
                KeyEvent::Vk {
                    vk: VK_LCONTROL,
                    up: true
                },
            ]
        );
    }

    #[test]
    fn partial_send_releases_held_keys_in_reverse() {
        let combo = KeyCombo::new(true, false, true, false, "Delete");
        let ev = combo_events(&combo);
        // Only ctrl down, alt down and delete down were delivered.
        let cleanup = cleanup_after_partial(&ev, 3);
        assert_eq!(
            cleanup,
            vec![
                KeyEvent::Vk { vk: 0x2E, up: true },
                KeyEvent::Vk {
                    vk: VK_LMENU,
                    up: true
                },
                KeyEvent::Vk {
                    vk: VK_LCONTROL,
                    up: true
                },
            ]
        );
        assert!(cleanup_after_partial(&ev, ev.len()).is_empty());
    }

    #[test]
    fn win_l_detected() {
        assert!(KeyCombo::new(false, false, false, true, "L").is_win_l());
        assert!(!KeyCombo::new(true, false, false, true, "L").is_win_l());
    }

    #[test]
    fn text_with_diacritics_and_emoji() {
        let ev = text_events("Př😀\r\nx");
        let units: Vec<u16> = ev
            .iter()
            .filter_map(|e| match e {
                KeyEvent::Unicode { unit, up: false } => Some(*unit),
                _ => None,
            })
            .collect();
        assert_eq!(units, vec![0x50, 0x159, 0xD83D, 0xDE00, 0x78]);
        let enters = ev
            .iter()
            .filter(|e| {
                **e == KeyEvent::Vk {
                    vk: VK_RETURN,
                    up: false,
                }
            })
            .count();
        assert_eq!(enters, 1);
    }

    #[test]
    fn mods_bits_roundtrip() {
        for b in 0..16u8 {
            assert_eq!(Mods::from_bits(b).bits(), b);
        }
    }
}
