//! The overlay state machine (SPEC 4): opening at the cursor, hold vs. tap, selection by
//! angle/cell, keyboard navigation, folders and running the chosen action.

use std::time::{Duration, Instant};

use crossbeam_channel::{select, Receiver};
use tauri::Emitter;

use super::geometry::{self, Hit, PanelGeometry, Rect};
use super::view::{self, OverlayView};
use super::window;
use crate::app::core;
use crate::foreground::{self, ForegroundInfo};
use crate::input::hook::{InputEvent, MouseBtn, NavKey, SHARED};
use crate::profiles::model::{Action, Button, Layout, Page};

pub enum Msg {
    /// The overlay frontend painted view `seq` (for latency measurement).
    Rendered {
        seq: u64,
    },
    ConfigChanged,
    ProfilesChanged,
    /// An action started from the panel failed: flash a red "!" on its button.
    ActionFailed {
        button_id: String,
        hint: Option<String>,
    },
    Close,
}

struct Open {
    target: Option<ForegroundInfo>,
    profile_id: String,
    path: Vec<usize>,
    page: Page,
    layout: Layout,
    geo: PanelGeometry,
    rect: Rect,
    scale: f64,
    selected: Option<i32>,
    /// Trigger still physically held.
    held: bool,
    down_at: Instant,
    /// Hold mode: releasing the trigger runs the selected button.
    hold_mode: bool,
    arrows: [bool; 4],
    /// Non-interactive flash after a failed action.
    flash_until: Option<Instant>,
}

pub struct Controller {
    open: Option<Open>,
    seq: u64,
    trigger_at: Option<Instant>,
    last_rect: Option<(Rect, f64, Layout, PanelGeometry, String, Vec<usize>)>,
    /// The window gets hidden at this time (after the close animation).
    pending_hide: Option<Instant>,
}

/// Length of the close animation; the window is hidden right after it.
const CLOSE_ANIMATION: Duration = Duration::from_millis(110);

/// Starts the controller thread consuming hook events and internal messages.
pub fn start(input: Receiver<InputEvent>, msgs: Receiver<Msg>) {
    std::thread::Builder::new()
        .name("overlay-controller".into())
        .spawn(move || {
            crate::audio::com_init();
            let mut c = Controller {
                open: None,
                seq: 0,
                trigger_at: None,
                last_rect: None,
                pending_hide: None,
            };
            loop {
                let deadline = [c.open.as_ref().and_then(|o| o.flash_until), c.pending_hide]
                    .into_iter()
                    .flatten()
                    .min();
                let timeout = deadline
                    .map(|t| t.saturating_duration_since(Instant::now()))
                    .unwrap_or(Duration::from_secs(3600));
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    select! {
                        recv(input) -> ev => match ev {
                            Ok(ev) => { c.on_input(ev); true }
                            Err(_) => false,
                        },
                        recv(msgs) -> m => match m {
                            Ok(m) => { c.on_msg(m); true }
                            Err(_) => false,
                        },
                        default(timeout) => { c.on_timeout(); true }
                    }
                }));
                match result {
                    Ok(true) => {}
                    Ok(false) => break,
                    Err(_) => {
                        tracing::error!("overlay controller panicked; closing the panel");
                        c.close_now();
                    }
                }
            }
        })
        .expect("spawn overlay controller");
}

fn hwnd() -> isize {
    core()
        .overlay_hwnd
        .load(std::sync::atomic::Ordering::SeqCst)
}

impl Controller {
    fn on_input(&mut self, ev: InputEvent) {
        if !matches!(ev, InputEvent::MouseMove { .. }) {
            tracing::debug!(?ev, "input event");
        }
        match ev {
            InputEvent::TriggerDown { at, x, y } => {
                if core().is_paused() {
                    return;
                }
                match &self.open {
                    Some(o) if o.flash_until.is_none() => self.close(),
                    _ => self.open_at(at, x, y),
                }
            }
            InputEvent::TriggerUp { at } => self.on_trigger_up(at),
            InputEvent::MouseMove { x, y } => self.on_mouse_move(x, y),
            InputEvent::MouseDown {
                button,
                x,
                y,
                inside,
            } => self.on_mouse_down(button, x, y, inside),
            InputEvent::Nav { key, down } => self.on_nav(key, down),
            InputEvent::Recorded(combo) => {
                if combo.win || combo.alt {
                    crate::input::send::send_mask_key();
                }
                let _ = core().app.emit("hotkey:recorded", &combo);
            }
        }
    }

    fn on_msg(&mut self, m: Msg) {
        match m {
            Msg::Rendered { seq } => {
                if seq == self.seq {
                    if let Some(t0) = self.trigger_at.take() {
                        let ms = t0.elapsed().as_secs_f64() * 1000.0;
                        tracing::info!(latency_ms = format!("{ms:.1}"), "overlay shown");
                        core().record_latency(ms);
                    }
                }
            }
            Msg::ConfigChanged | Msg::ProfilesChanged => {
                if self.open.as_ref().is_some_and(|o| o.flash_until.is_none()) {
                    self.reload_page();
                }
            }
            Msg::ActionFailed { button_id, hint } => self.flash_alert(button_id, hint),
            Msg::Close => self.close(),
        }
    }

    fn on_timeout(&mut self) {
        let now = Instant::now();
        if self.pending_hide.is_some_and(|t| now >= t) {
            self.pending_hide = None;
            window::hide(hwnd());
        }
        if let Some(o) = &self.open {
            if o.flash_until.is_some_and(|t| now >= t) {
                self.close();
            }
        }
    }

    fn open_at(&mut self, at: Instant, x: i32, y: i32) {
        let core = core();
        let target = foreground::current();
        let exe = target
            .as_ref()
            .filter(|t| !t.is_own_process())
            .and_then(|t| t.exe.clone())
            .or_else(|| {
                core.last_foreground
                    .lock()
                    .as_ref()
                    .and_then(|f| f.exe.clone())
            });
        let profile_id = core.profile_id_for(exe.as_deref());
        let (layout, page) = {
            let store = core.profiles.read();
            let profile = store
                .get(&profile_id)
                .unwrap_or_else(|| store.default_profile());
            (
                profile.layout,
                store.effective_page(profile, &[]).unwrap_or_default(),
            )
        };
        let size = core.config.read().appearance.size;
        let geo = geometry::panel_geometry(layout, size);
        let mon = window::monitor_at(x, y);
        let px = (
            (geo.width * mon.scale).round() as i32,
            (geo.height * mon.scale).round() as i32,
        );
        let rect = geometry::place((x, y), px, mon.work);

        self.trigger_at = Some(at);
        self.pending_hide = None;
        self.open = Some(Open {
            target,
            profile_id,
            path: Vec::new(),
            page,
            layout,
            geo,
            rect,
            scale: mon.scale,
            selected: None,
            held: true,
            down_at: at,
            hold_mode: false,
            arrows: [false; 4],
            flash_until: None,
        });
        // Masks a lone Win/Alt release after the swallowed trigger key.
        let mods = core.config.read().hotkey.mods();
        if mods.win || mods.alt {
            crate::input::send::send_mask_key();
        }
        let h = hwnd();
        window::position(h, rect);
        SHARED.set_overlay(true, Some((rect.left, rect.top, rect.right, rect.bottom)));
        self.emit_view(None);
        window::show(h, rect);
        tracing::debug!(?rect, scale = mon.scale, "panel opened");
    }

    fn emit_view(&mut self, alert: Option<(String, Option<String>)>) {
        let Some(o) = &self.open else { return };
        self.seq += 1;
        let core = core();
        let profile_name = core
            .profiles
            .read()
            .get(&o.profile_id)
            .map(|p| p.name.clone())
            .unwrap_or_default();
        let (alert, hint) = match alert {
            Some((a, h)) => (Some(a), h),
            None => (None, None),
        };
        let v = OverlayView {
            seq: self.seq,
            layout: o.layout,
            geometry: o.geo,
            buttons: view::page_views(&o.page),
            in_folder: !o.path.is_empty(),
            selected: o.selected,
            appearance: core.config.read().appearance.clone(),
            profile_name,
            alert,
            hint,
        };
        let _ = core.app.emit_to(window::OVERLAY_LABEL, "overlay:show", &v);
    }

    fn emit_selection(&self) {
        if let Some(o) = &self.open {
            let _ = core().app.emit_to(
                window::OVERLAY_LABEL,
                "overlay:select",
                serde_json::json!({ "seq": self.seq, "selected": o.selected }),
            );
        }
    }

    /// Closes with the fade-out animation (if enabled); the window hides after it.
    pub fn close(&mut self) {
        if self.open.take().is_none() {
            return;
        }
        SHARED.set_overlay(false, None);
        let animate = core().config.read().appearance.animations;
        let _ = core().app.emit_to(
            window::OVERLAY_LABEL,
            "overlay:hide",
            serde_json::json!({ "animate": animate }),
        );
        if animate {
            self.pending_hide = Some(Instant::now() + CLOSE_ANIMATION);
        } else {
            window::hide(hwnd());
        }
    }

    /// Hides immediately, without animation (before running an action, SPEC 4.5).
    pub fn close_now(&mut self) {
        self.open = None;
        self.pending_hide = None;
        window::hide(hwnd());
        SHARED.set_overlay(false, None);
        let _ = core().app.emit_to(
            window::OVERLAY_LABEL,
            "overlay:hide",
            serde_json::json!({ "animate": false }),
        );
    }

    fn reload_page(&mut self) {
        let Some(o) = &mut self.open else { return };
        let core = core();
        let store = core.profiles.read();
        let Some(profile) = store.get(&o.profile_id) else {
            drop(store);
            self.close();
            return;
        };
        match store.effective_page(profile, &o.path) {
            Some(p) => o.page = p,
            None => {
                o.path.clear();
                o.page = store.effective_page(profile, &[]).unwrap_or_default();
            }
        }
        drop(store);
        self.emit_view(None);
    }

    /// Hit test of a physical screen point.
    fn hit(&self, x: i32, y: i32, limit_radius: bool) -> Hit {
        let Some(o) = &self.open else {
            return Hit::None;
        };
        let lx = (x - o.rect.left) as f64 / o.scale;
        let ly = (y - o.rect.top) as f64 / o.scale;
        match o.layout {
            Layout::Radial => {
                let c = o.geo.width / 2.0;
                geometry::radial_hit(
                    lx - c,
                    ly - c,
                    o.geo.inner,
                    limit_radius.then_some(o.geo.outer + 4.0),
                )
            }
            Layout::Grid { .. } => geometry::grid_hit(lx, ly, &o.geo),
        }
    }

    fn set_selected(&mut self, sel: Option<i32>) {
        if let Some(o) = &mut self.open {
            if o.selected != sel {
                o.selected = sel;
                self.emit_selection();
            }
        }
    }

    fn hit_to_sel(h: Hit) -> Option<i32> {
        match h {
            Hit::Center => Some(-1),
            Hit::Slot(i) => Some(i as i32),
            Hit::None => None,
        }
    }

    fn on_mouse_move(&mut self, x: i32, y: i32) {
        let Some(o) = &self.open else { return };
        if o.flash_until.is_some() {
            return;
        }
        let limit = !o.held;
        let sel = Self::hit_to_sel(self.hit(x, y, limit));
        self.set_selected(sel);
    }

    fn on_mouse_down(&mut self, button: MouseBtn, x: i32, y: i32, inside: bool) {
        let Some(o) = &self.open else { return };
        if o.flash_until.is_some() {
            if inside {
                self.close();
            }
            return;
        }
        if !inside {
            self.close();
            return;
        }
        if button == MouseBtn::Right {
            self.back_or_close();
            return;
        }
        match self.hit(x, y, true) {
            Hit::Center => self.back_or_close(),
            Hit::Slot(i) => self.activate(i),
            Hit::None => self.close(),
        }
    }

    fn on_trigger_up(&mut self, at: Instant) {
        let threshold = core().config.read().hold_threshold_ms as u128;
        let Some(o) = &mut self.open else { return };
        if !o.held || o.flash_until.is_some() {
            return;
        }
        o.held = false;
        let held_ms = at.duration_since(o.down_at).as_millis();
        if held_ms < threshold {
            // Tap: the panel stays open for mouse / keyboard.
            return;
        }
        o.hold_mode = true;
        match o.selected {
            Some(i) if i >= 0 => self.activate(i as usize),
            _ => self.close(),
        }
    }

    fn on_nav(&mut self, key: NavKey, down: bool) {
        let Some(o) = &mut self.open else { return };
        if o.flash_until.is_some() {
            if down && key == NavKey::Escape {
                self.close();
            }
            return;
        }
        let arrow_idx = match key {
            NavKey::Up => Some(0),
            NavKey::Down => Some(1),
            NavKey::Left => Some(2),
            NavKey::Right => Some(3),
            _ => None,
        };
        if let Some(idx) = arrow_idx {
            match o.layout {
                Layout::Radial => {
                    o.arrows[idx] = down;
                    if down {
                        let [u, d, l, r] = o.arrows;
                        if let Some(s) = geometry::radial_slot_from_arrows(u, d, l, r) {
                            self.set_selected(Some(s as i32));
                        }
                    }
                }
                Layout::Grid { .. } => {
                    if down {
                        let (dx, dy) = [(0, -1), (0, 1), (-1, 0), (1, 0)][idx];
                        let cur = o.selected.filter(|s| *s >= 0).map(|s| s as usize);
                        let next = geometry::grid_move(
                            cur,
                            o.geo.cols as usize,
                            o.geo.rows as usize,
                            dx,
                            dy,
                        );
                        self.set_selected(Some(next as i32));
                    }
                }
            }
            return;
        }
        if !down {
            return;
        }
        match key {
            NavKey::Escape => self.close(),
            NavKey::Back => self.back_or_close(),
            NavKey::Enter => match o.selected {
                Some(-1) => self.back_or_close(),
                Some(i) if i >= 0 => self.activate(i as usize),
                _ => {}
            },
            _ => {}
        }
    }

    fn back_or_close(&mut self) {
        let Some(o) = &mut self.open else { return };
        if o.path.pop().is_some() {
            o.selected = None;
            self.reload_page();
        } else {
            self.close();
        }
    }

    fn is_grid_back_slot(o: &Open, slot: usize) -> bool {
        matches!(o.layout, Layout::Grid { .. }) && !o.path.is_empty() && slot == 0
    }

    fn activate(&mut self, slot: usize) {
        let Some(o) = &mut self.open else { return };
        if Self::is_grid_back_slot(o, slot) {
            self.back_or_close();
            return;
        }
        let Some(button) = o.page.slots.get(slot).cloned().flatten() else {
            // Empty slot: in hold mode releasing over nothing does nothing.
            if o.hold_mode {
                self.close();
            }
            return;
        };
        if let Action::Folder { .. } = button.action {
            o.path.push(slot);
            o.selected = None;
            o.hold_mode = false;
            self.reload_page();
            return;
        }
        let target = o.target.clone();
        self.last_rect = Some((
            o.rect,
            o.scale,
            o.layout,
            o.geo,
            o.profile_id.clone(),
            o.path.clone(),
        ));
        // SPEC 4.5: hide first, then restore focus, wait ~30 ms, then run.
        self.close_now();
        run_button(button, target);
    }

    /// Re-shows the panel briefly (non-interactive) with a red "!" on the failed button.
    fn flash_alert(&mut self, button_id: String, hint: Option<String>) {
        if self.open.is_some() {
            return;
        }
        let Some((rect, scale, layout, geo, profile_id, path)) = self.last_rect.clone() else {
            return;
        };
        let page = {
            let store = core().profiles.read();
            match store.get(&profile_id) {
                Some(p) => store.effective_page(p, &path).unwrap_or_default(),
                None => return,
            }
        };
        let duration = if hint.is_some() { 4000 } else { 1200 };
        self.pending_hide = None;
        self.open = Some(Open {
            target: None,
            profile_id,
            path,
            page,
            layout,
            geo,
            rect,
            scale,
            selected: None,
            held: false,
            down_at: Instant::now(),
            hold_mode: false,
            arrows: [false; 4],
            flash_until: Some(Instant::now() + Duration::from_millis(duration)),
        });
        SHARED.set_overlay(true, Some((rect.left, rect.top, rect.right, rect.bottom)));
        self.emit_view(Some((button_id, hint)));
        window::show(hwnd(), rect);
    }
}

/// Runs a button's action on a worker thread (SPEC 4.5 steps 3–4).
pub fn run_button(button: Button, target: Option<ForegroundInfo>) {
    std::thread::Builder::new()
        .name("action".into())
        .spawn(move || {
            crate::audio::com_init();
            let ctx = crate::actions::ActionCtx {
                target,
                button_id: Some(button.id.clone()),
                from_panel: true,
            };
            if let Err(e) = crate::actions::run_button_action(&button, &ctx) {
                tracing::error!(button = %button.title, action = button.action.type_name(), "action failed: {e:#}");
                let hint = crate::actions::uipi_hint_for(&e);
                let _ = core().controller.send(Msg::ActionFailed {
                    button_id: button.id.clone(),
                    hint,
                });
            }
        })
        .ok();
}
