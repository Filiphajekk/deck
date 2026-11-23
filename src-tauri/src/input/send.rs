//! Synthetic keyboard input through `SendInput`.

use std::time::{Duration, Instant};

use anyhow::bail;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, MapVirtualKeyW, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
    KEYBD_EVENT_FLAGS, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, MAPVK_VK_TO_VSC,
    VIRTUAL_KEY,
};

use crate::keys::{self, KeyCombo, KeyEvent};

/// `dwExtraInfo` marker on everything we inject, so our own hook ignores it.
pub const INJECTED_TAG: usize = 0x00DE_C1E5;

fn to_input(ev: KeyEvent) -> INPUT {
    let ki = match ev {
        KeyEvent::Vk { vk, up } => {
            let scan = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) } as u16;
            let mut flags = KEYBD_EVENT_FLAGS(0);
            if up {
                flags |= KEYEVENTF_KEYUP;
            }
            if keys::is_extended(vk) {
                flags |= KEYEVENTF_EXTENDEDKEY;
            }
            KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: INJECTED_TAG,
            }
        }
        KeyEvent::Unicode { unit, up } => {
            let mut flags = KEYEVENTF_UNICODE;
            if up {
                flags |= KEYEVENTF_KEYUP;
            }
            KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: unit,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: INJECTED_TAG,
            }
        }
    };
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki },
    }
}

fn send_raw(events: &[KeyEvent]) -> usize {
    if events.is_empty() {
        return 0;
    }
    let inputs: Vec<INPUT> = events.iter().copied().map(to_input).collect();
    unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) as usize }
}

/// Sends events in one batch. If the batch is cut short (UIPI, desktop switch), every key
/// that was pressed gets released again so no modifier stays stuck.
pub fn send_events(events: &[KeyEvent]) -> anyhow::Result<()> {
    let delivered = send_raw(events);
    if delivered < events.len() {
        let cleanup = keys::cleanup_after_partial(events, delivered);
        send_raw(&cleanup);
        bail!(
            "SendInput delivered {delivered} of {} events (input blocked, e.g. by an elevated window)",
            events.len()
        );
    }
    Ok(())
}

/// Physically held modifiers would combine with our synthetic keys (Ctrl+A would become
/// Ctrl+Shift+A while the user still holds the trigger). Wait briefly for the user to let
/// go, then release whatever is still down.
pub fn release_held_modifiers(max_wait: Duration) {
    const MODS: [u16; 8] = [
        keys::VK_LCONTROL,
        keys::VK_RCONTROL,
        keys::VK_LSHIFT,
        keys::VK_RSHIFT,
        keys::VK_LMENU,
        keys::VK_RMENU,
        keys::VK_LWIN,
        keys::VK_RWIN,
    ];
    let held = || -> Vec<u16> {
        MODS.iter()
            .copied()
            .filter(|&vk| unsafe { GetAsyncKeyState(vk as i32) } as u16 & 0x8000 != 0)
            .collect()
    };
    let start = Instant::now();
    let mut still = held();
    while !still.is_empty() && start.elapsed() < max_wait {
        std::thread::sleep(Duration::from_millis(10));
        still = held();
    }
    if !still.is_empty() {
        tracing::debug!(?still, "releasing modifiers still held by the user");
        let mut ups: Vec<KeyEvent> = still
            .iter()
            .map(|&vk| KeyEvent::Vk { vk, up: true })
            .collect();
        // A lone Win/Alt release would open Start or the menu bar; mask it.
        if still.iter().any(|&v| {
            matches!(
                v,
                keys::VK_LWIN | keys::VK_RWIN | keys::VK_LMENU | keys::VK_RMENU
            )
        }) {
            ups.insert(
                0,
                KeyEvent::Vk {
                    vk: keys::VK_MASK,
                    up: false,
                },
            );
            ups.insert(
                1,
                KeyEvent::Vk {
                    vk: keys::VK_MASK,
                    up: true,
                },
            );
        }
        send_raw(&ups);
    }
}

/// Sends a dummy key so that releasing Win or Alt does not open Start / the menu bar.
pub fn send_mask_key() {
    send_raw(&[
        KeyEvent::Vk {
            vk: keys::VK_MASK,
            up: false,
        },
        KeyEvent::Vk {
            vk: keys::VK_MASK,
            up: true,
        },
    ]);
}

/// Sends a sequence of key combinations with a short pause between them.
pub fn send_combos(combos: &[KeyCombo]) -> anyhow::Result<()> {
    for (i, combo) in combos.iter().enumerate() {
        if i > 0 {
            std::thread::sleep(Duration::from_millis(30));
        }
        if combo.is_win_l() {
            crate::actions::system::lock()?;
            continue;
        }
        if combo.vk().is_none() {
            bail!("unknown key in combination {combo}");
        }
        send_events(&keys::combo_events(combo))?;
    }
    Ok(())
}

/// Types text with KEYEVENTF_UNICODE (diacritics and emoji work regardless of layout).
pub fn type_text(text: &str) -> anyhow::Result<()> {
    let events = keys::text_events(text);
    // Large batches are split so slow targets do not drop characters.
    for chunk in events.chunks(200) {
        send_events(chunk)?;
        if events.len() > 200 {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    Ok(())
}

/// Presses and releases a single virtual key (media keys etc.).
pub fn tap_vk(vk: u16) -> anyhow::Result<()> {
    send_events(&[
        KeyEvent::Vk { vk, up: false },
        KeyEvent::Vk { vk, up: true },
    ])
}
