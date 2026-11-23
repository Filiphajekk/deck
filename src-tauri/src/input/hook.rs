//! Low-level keyboard and mouse hooks (`WH_KEYBOARD_LL`, `WH_MOUSE_LL`).
//!
//! The hooks run on their own thread with a message loop. The callbacks only read atomics,
//! decide whether to swallow the event and push a small message into a channel; all real
//! work happens on the consumer side. Windows silently unhooks callbacks that take too long.

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU16, AtomicU32, AtomicU8, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{
    GetCurrentThread, GetCurrentThreadId, SetThreadPriority, THREAD_PRIORITY_HIGHEST,
};
use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW, SetWindowsHookExW,
    TranslateMessage, UnhookWindowsHookEx, HC_ACTION, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT,
    WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN,
    WM_MBUTTONUP, WM_MOUSEMOVE, WM_QUIT, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN,
    WM_XBUTTONDOWN, WM_XBUTTONUP,
};

use super::send::INJECTED_TAG;
use crate::keys::{self, KeyCombo, Mods};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavKey {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Escape,
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseBtn {
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub enum InputEvent {
    TriggerDown {
        at: Instant,
        x: i32,
        y: i32,
    },
    TriggerUp {
        at: Instant,
    },
    Nav {
        key: NavKey,
        down: bool,
    },
    MouseMove {
        x: i32,
        y: i32,
    },
    MouseDown {
        button: MouseBtn,
        x: i32,
        y: i32,
        inside: bool,
    },
    Recorded(KeyCombo),
}

/// Mouse buttons that can open the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MouseTrigger {
    #[default]
    None,
    Middle,
    X1,
    X2,
}

impl MouseTrigger {
    fn code(self) -> u8 {
        match self {
            MouseTrigger::None => 0,
            MouseTrigger::Middle => 1,
            MouseTrigger::X1 => 2,
            MouseTrigger::X2 => 3,
        }
    }
}

/// State shared between the hook callbacks and the rest of the app.
pub struct HookShared {
    hotkey_vk: AtomicU16,
    hotkey_mods: AtomicU8,
    mouse_trigger: AtomicU8,
    pub paused: AtomicBool,
    /// Panel is visible: navigation keys and clicks inside `rect` are swallowed.
    pub overlay_open: AtomicBool,
    rect: [AtomicI32; 4],
    recording: AtomicBool,
    // Hook-thread-only bookkeeping (atomics so the statics are Sync).
    trigger_active: AtomicBool,
    trigger_up_sent: AtomicBool,
    mouse_trigger_active: AtomicBool,
    swallow_key_up: AtomicU16,
    swallow_button_up: AtomicBool,
    hook_thread_id: AtomicU32,
    last_event_ms: AtomicU32,
}

pub static SHARED: HookShared = HookShared {
    hotkey_vk: AtomicU16::new(0),
    hotkey_mods: AtomicU8::new(0),
    mouse_trigger: AtomicU8::new(0),
    paused: AtomicBool::new(false),
    overlay_open: AtomicBool::new(false),
    rect: [
        AtomicI32::new(0),
        AtomicI32::new(0),
        AtomicI32::new(0),
        AtomicI32::new(0),
    ],
    recording: AtomicBool::new(false),
    trigger_active: AtomicBool::new(false),
    trigger_up_sent: AtomicBool::new(false),
    mouse_trigger_active: AtomicBool::new(false),
    swallow_key_up: AtomicU16::new(0),
    swallow_button_up: AtomicBool::new(false),
    hook_thread_id: AtomicU32::new(0),
    last_event_ms: AtomicU32::new(0),
};

static SENDER: OnceLock<Sender<InputEvent>> = OnceLock::new();

/// Number of keyboard / mouse callbacks seen (diagnostics, logged by the watchdog).
pub static KB_CALLS: AtomicU32 = AtomicU32::new(0);
pub static MOUSE_CALLS: AtomicU32 = AtomicU32::new(0);

impl HookShared {
    pub fn set_hotkey(&self, combo: &KeyCombo) {
        let vk = combo.vk().unwrap_or(0);
        self.hotkey_mods
            .store(combo.mods().bits(), Ordering::SeqCst);
        self.hotkey_vk.store(vk, Ordering::SeqCst);
    }

    pub fn set_mouse_trigger(&self, t: MouseTrigger) {
        self.mouse_trigger.store(t.code(), Ordering::SeqCst);
    }

    pub fn set_overlay(&self, open: bool, rect: Option<(i32, i32, i32, i32)>) {
        if let Some((l, t, r, b)) = rect {
            self.rect[0].store(l, Ordering::SeqCst);
            self.rect[1].store(t, Ordering::SeqCst);
            self.rect[2].store(r, Ordering::SeqCst);
            self.rect[3].store(b, Ordering::SeqCst);
        }
        self.overlay_open.store(open, Ordering::SeqCst);
    }

    /// Next key combination pressed is captured (and swallowed) instead of being delivered.
    pub fn start_recording(&self) {
        self.recording.store(true, Ordering::SeqCst);
    }

    pub fn cancel_recording(&self) {
        self.recording.store(false, Ordering::SeqCst);
    }

    fn inside(&self, x: i32, y: i32) -> bool {
        let l = self.rect[0].load(Ordering::Relaxed);
        let t = self.rect[1].load(Ordering::Relaxed);
        let r = self.rect[2].load(Ordering::Relaxed);
        let b = self.rect[3].load(Ordering::Relaxed);
        x >= l && x < r && y >= t && y < b
    }

    /// Milliseconds (GetTickCount based) of the last event the hook saw; used for health checks.
    pub fn last_event_ms(&self) -> u32 {
        self.last_event_ms.load(Ordering::Relaxed)
    }
}

fn send(ev: InputEvent) {
    if let Some(tx) = SENDER.get() {
        let _ = tx.send(ev);
    }
}

fn key_down(vk: u16) -> bool {
    (unsafe { GetAsyncKeyState(vk as i32) } as u16) & 0x8000 != 0
}

/// Current modifier state. In a LL hook the async state of previously pressed keys is
/// already up to date, only the key being processed is not.
fn current_mods() -> Mods {
    Mods {
        ctrl: key_down(keys::VK_CONTROL),
        shift: key_down(keys::VK_SHIFT),
        alt: key_down(keys::VK_MENU),
        win: key_down(keys::VK_LWIN) || key_down(keys::VK_RWIN),
    }
}

fn modifier_in(mods: Mods, vk: u16) -> bool {
    match vk {
        keys::VK_CONTROL | keys::VK_LCONTROL | keys::VK_RCONTROL => mods.ctrl,
        keys::VK_SHIFT | keys::VK_LSHIFT | keys::VK_RSHIFT => mods.shift,
        keys::VK_MENU | keys::VK_LMENU | keys::VK_RMENU => mods.alt,
        keys::VK_LWIN | keys::VK_RWIN => mods.win,
        _ => false,
    }
}

fn nav_key(vk: u16) -> Option<NavKey> {
    Some(match vk {
        keys::VK_UP => NavKey::Up,
        keys::VK_DOWN => NavKey::Down,
        keys::VK_LEFT => NavKey::Left,
        keys::VK_RIGHT => NavKey::Right,
        keys::VK_RETURN => NavKey::Enter,
        keys::VK_ESCAPE => NavKey::Escape,
        keys::VK_BACK => NavKey::Back,
        _ => return None,
    })
}

/// Returns true when the event must be swallowed.
fn handle_keyboard(msg: u32, kb: &KBDLLHOOKSTRUCT) -> bool {
    let s = &SHARED;
    s.last_event_ms.store(kb.time, Ordering::Relaxed);
    KB_CALLS.fetch_add(1, Ordering::Relaxed);
    if kb.dwExtraInfo == INJECTED_TAG {
        return false;
    }
    let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
    let vk = kb.vkCode as u16;

    // Key-up of a key whose key-down we swallowed (recorder).
    if !down && s.swallow_key_up.load(Ordering::Relaxed) == vk && vk != 0 {
        s.swallow_key_up.store(0, Ordering::Relaxed);
        return true;
    }

    if s.recording.load(Ordering::Relaxed) {
        if down && !keys::is_modifier_vk(vk) {
            if let Some(name) = keys::name_from_vk(vk) {
                let m = current_mods();
                s.recording.store(false, Ordering::SeqCst);
                s.swallow_key_up.store(vk, Ordering::Relaxed);
                send(InputEvent::Recorded(KeyCombo {
                    ctrl: m.ctrl,
                    shift: m.shift,
                    alt: m.alt,
                    win: m.win,
                    key: Some(name),
                }));
                return true;
            }
        }
        return false;
    }

    let hk_vk = s.hotkey_vk.load(Ordering::Relaxed);
    let hk_mods = Mods::from_bits(s.hotkey_mods.load(Ordering::Relaxed));
    let active = s.trigger_active.load(Ordering::Relaxed);

    if hk_vk != 0 && vk == hk_vk {
        if down {
            if active {
                return true; // auto-repeat while held
            }
            if !s.paused.load(Ordering::Relaxed) && current_mods() == hk_mods {
                s.trigger_active.store(true, Ordering::Relaxed);
                s.trigger_up_sent.store(false, Ordering::Relaxed);
                let (x, y) = cursor_pos();
                send(InputEvent::TriggerDown {
                    at: Instant::now(),
                    x,
                    y,
                });
                return true;
            }
        } else if active {
            s.trigger_active.store(false, Ordering::Relaxed);
            if !s.trigger_up_sent.swap(true, Ordering::Relaxed) {
                send(InputEvent::TriggerUp { at: Instant::now() });
            }
            return true;
        }
    }

    // Releasing one of the hotkey's modifiers also ends a "hold".
    if !down
        && active
        && modifier_in(hk_mods, vk)
        && !s.trigger_up_sent.swap(true, Ordering::Relaxed)
    {
        send(InputEvent::TriggerUp { at: Instant::now() });
        return false;
    }

    if s.overlay_open.load(Ordering::Relaxed) {
        if let Some(key) = nav_key(vk) {
            send(InputEvent::Nav { key, down });
            return true;
        }
    }
    false
}

fn cursor_pos() -> (i32, i32) {
    let mut p = windows::Win32::Foundation::POINT::default();
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut p);
    }
    (p.x, p.y)
}

fn handle_mouse(msg: u32, ms: &MSLLHOOKSTRUCT) -> bool {
    let s = &SHARED;
    if ms.dwExtraInfo == INJECTED_TAG {
        return false;
    }
    s.last_event_ms.store(ms.time, Ordering::Relaxed);
    MOUSE_CALLS.fetch_add(1, Ordering::Relaxed);
    let (x, y) = (ms.pt.x, ms.pt.y);
    let open = s.overlay_open.load(Ordering::Relaxed);
    match msg {
        WM_MOUSEMOVE => {
            if open {
                send(InputEvent::MouseMove { x, y });
            }
            false
        }
        WM_MBUTTONDOWN | WM_MBUTTONUP | WM_XBUTTONDOWN | WM_XBUTTONUP => {
            let code = match msg {
                WM_MBUTTONDOWN | WM_MBUTTONUP => 1,
                _ => match (ms.mouseData >> 16) as u16 {
                    1 => 2,
                    2 => 3,
                    _ => 0,
                },
            };
            let configured = s.mouse_trigger.load(Ordering::Relaxed);
            if configured == 0 || code != configured {
                return false;
            }
            let down = msg == WM_MBUTTONDOWN || msg == WM_XBUTTONDOWN;
            if down {
                if s.paused.load(Ordering::Relaxed) {
                    return false;
                }
                s.mouse_trigger_active.store(true, Ordering::Relaxed);
                send(InputEvent::TriggerDown {
                    at: Instant::now(),
                    x,
                    y,
                });
                true
            } else if s.mouse_trigger_active.swap(false, Ordering::Relaxed) {
                send(InputEvent::TriggerUp { at: Instant::now() });
                true
            } else {
                false
            }
        }
        WM_LBUTTONDOWN | WM_RBUTTONDOWN => {
            if !open {
                return false;
            }
            let inside = s.inside(x, y);
            let button = if msg == WM_LBUTTONDOWN {
                MouseBtn::Left
            } else {
                MouseBtn::Right
            };
            send(InputEvent::MouseDown {
                button,
                x,
                y,
                inside,
            });
            if inside {
                s.swallow_button_up.store(true, Ordering::Relaxed);
            }
            inside
        }
        WM_LBUTTONUP | WM_RBUTTONUP => s.swallow_button_up.swap(false, Ordering::Relaxed),
        _ => false,
    }
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        let msg = wparam.0 as u32;
        // Never unwind across the FFI boundary; a panic just lets the event through.
        let swallow = std::panic::catch_unwind(|| handle_keyboard(msg, kb)).unwrap_or(false);
        if swallow {
            return LRESULT(1);
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let ms = &*(lparam.0 as *const MSLLHOOKSTRUCT);
        let msg = wparam.0 as u32;
        let swallow = std::panic::catch_unwind(|| handle_mouse(msg, ms)).unwrap_or(false);
        if swallow {
            return LRESULT(1);
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

fn run_hook_thread() -> anyhow::Result<()> {
    unsafe {
        let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_HIGHEST);
        let hinst = GetModuleHandleW(None)?;
        let kb = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), Some(hinst.into()), 0)?;
        let ms = match SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), Some(hinst.into()), 0) {
            Ok(h) => h,
            Err(e) => {
                let _ = UnhookWindowsHookEx(kb);
                return Err(e.into());
            }
        };
        SHARED
            .hook_thread_id
            .store(GetCurrentThreadId(), Ordering::SeqCst);
        SHARED.last_event_ms.store(
            windows::Win32::System::SystemInformation::GetTickCount(),
            Ordering::Relaxed,
        );
        tracing::info!("input hooks installed");
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let _ = UnhookWindowsHookEx(kb);
        let _ = UnhookWindowsHookEx(ms);
        tracing::info!("input hooks removed");
    }
    Ok(())
}

/// Starts the hook thread under a supervisor that restarts it if it dies (SPEC 10.3).
pub fn start() -> Receiver<InputEvent> {
    let (tx, rx) = crossbeam_channel::unbounded();
    let _ = SENDER.set(tx);
    std::thread::Builder::new()
        .name("input-hook-supervisor".into())
        .spawn(|| loop {
            let handle = std::thread::Builder::new()
                .name("input-hook".into())
                .spawn(|| {
                    if let Err(e) = run_hook_thread() {
                        tracing::error!("input hook thread failed: {e:#}");
                    }
                });
            match handle {
                Ok(h) => {
                    if h.join().is_err() {
                        tracing::error!("input hook thread panicked, restarting");
                    } else if SHUTTING_DOWN.load(Ordering::SeqCst) {
                        return;
                    } else {
                        tracing::warn!("input hook thread exited, restarting");
                    }
                }
                Err(e) => tracing::error!("cannot spawn input hook thread: {e}"),
            }
            std::thread::sleep(Duration::from_millis(300));
        })
        .expect("spawn hook supervisor");
    start_watchdog();
    rx
}

static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);

/// Windows silently removes low-level hooks whose callback was too slow (e.g. the machine
/// was under heavy load). The watchdog compares the system's last input time with the last
/// event our hook saw and reinstalls the hooks when they clearly stopped receiving input.
fn start_watchdog() {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    std::thread::Builder::new()
        .name("input-hook-watchdog".into())
        .spawn(|| {
            let mut stale = 0;
            loop {
                std::thread::sleep(Duration::from_secs(2));
                if SHUTTING_DOWN.load(Ordering::SeqCst) {
                    return;
                }
                let mut lii = LASTINPUTINFO {
                    cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
                    dwTime: 0,
                };
                if !unsafe { GetLastInputInfo(&mut lii) }.as_bool() {
                    continue;
                }
                let seen = SHARED.last_event_ms();
                tracing::trace!(
                    kb = KB_CALLS.load(Ordering::Relaxed),
                    mouse = MOUSE_CALLS.load(Ordering::Relaxed),
                    last_input = lii.dwTime,
                    seen,
                    "hook health"
                );
                // Signed difference handles the 49-day tick wrap-around.
                let behind = lii.dwTime.wrapping_sub(seen) as i32;
                if seen != 0 && behind > 1500 {
                    stale += 1;
                } else {
                    stale = 0;
                }
                if stale >= 2 {
                    tracing::warn!(
                        behind_ms = behind,
                        "input hooks stopped receiving events; reinstalling"
                    );
                    stale = 0;
                    restart_hook_thread();
                }
            }
        })
        .expect("spawn hook watchdog");
}

fn restart_hook_thread() {
    let tid = SHARED.hook_thread_id.swap(0, Ordering::SeqCst);
    if tid != 0 {
        unsafe {
            let _ = PostThreadMessageW(tid, WM_QUIT, WPARAM(0), LPARAM(0));
        }
    }
}

/// Asks the hook thread to exit (on app shutdown).
pub fn stop() {
    SHUTTING_DOWN.store(true, Ordering::SeqCst);
    let tid = SHARED.hook_thread_id.load(Ordering::SeqCst);
    if tid != 0 {
        unsafe {
            let _ = PostThreadMessageW(tid, WM_QUIT, WPARAM(0), LPARAM(0));
        }
    }
}
