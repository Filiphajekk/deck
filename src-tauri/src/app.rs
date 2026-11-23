//! Global application core shared by Tauri commands, the overlay controller and actions.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Arc, OnceLock};

use crossbeam_channel::Sender;
use parking_lot::{Mutex, RwLock};
use tauri::{AppHandle, Emitter};

use crate::config::{self, Config};
use crate::input::hook::SHARED;
use crate::lang::{self, Lang};
use crate::overlay::controller::Msg;
use crate::paths;
use crate::profiles::store::ProfileStore;

pub struct Core {
    pub app: AppHandle,
    pub config: RwLock<Config>,
    pub profiles: RwLock<ProfileStore>,
    pub paused: AtomicBool,
    /// Current state (0 = A, 1 = B) of `toggle` buttons by button id.
    pub toggles: Mutex<HashMap<String, u8>>,
    /// Profile chosen with a `switchProfile` action; cleared when the foreground app changes.
    pub manual_profile: Mutex<Option<String>>,
    /// The last foreground window that is not ours.
    pub last_foreground: Mutex<Option<crate::foreground::ForegroundInfo>>,
    /// Latest overlay show latencies in ms (SPEC 10.1).
    pub latencies: Mutex<VecDeque<f64>>,
    pub overlay_hwnd: AtomicIsize,
    pub controller: Sender<Msg>,
}

static CORE: OnceLock<Arc<Core>> = OnceLock::new();

/// The global core. Panics if called before `init`.
pub fn core() -> &'static Arc<Core> {
    CORE.get().expect("core initialised")
}

pub fn try_core() -> Option<&'static Arc<Core>> {
    CORE.get()
}

pub fn init(app: AppHandle, controller: Sender<Msg>) -> &'static Arc<Core> {
    let mut cfg = config::load(&paths::config_file());
    cfg.sanitize();
    let lang = lang::resolve(cfg.language);
    let profiles = ProfileStore::load(&paths::profiles_dir(), || {
        crate::profiles::defaults::default_profile(lang)
    });
    let core = Arc::new(Core {
        app,
        config: RwLock::new(cfg),
        profiles: RwLock::new(profiles),
        paused: AtomicBool::new(false),
        toggles: Mutex::new(HashMap::new()),
        manual_profile: Mutex::new(None),
        last_foreground: Mutex::new(None),
        latencies: Mutex::new(VecDeque::new()),
        overlay_hwnd: AtomicIsize::new(0),
        controller,
    });
    let _ = CORE.set(core);
    let core = CORE.get().unwrap();
    core.apply_config_to_hooks();
    core
}

impl Core {
    pub fn lang(&self) -> Lang {
        lang::resolve(self.config.read().language)
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    /// Pushes hotkey and mouse trigger settings to the hook thread.
    pub fn apply_config_to_hooks(&self) {
        let cfg = self.config.read();
        SHARED.set_hotkey(&cfg.hotkey);
        SHARED.set_mouse_trigger(cfg.mouse_trigger);
        SHARED.paused.store(self.is_paused(), Ordering::SeqCst);
    }

    /// Saves a new config, applies it and notifies windows.
    pub fn update_config(&self, mut new: Config) -> anyhow::Result<()> {
        new.sanitize();
        config::save(&paths::config_file(), &new)?;
        let old_lang = self.lang();
        let old_autostart = self.config.read().autostart;
        *self.config.write() = new.clone();
        self.apply_config_to_hooks();
        if self.lang() != old_lang {
            crate::tray::refresh(&self.app, self.lang(), self.is_paused());
        }
        if new.autostart != old_autostart {
            sync_autostart(&self.app);
        }
        if new.obs.enabled {
            crate::obs::ensure_started();
        }
        let _ = self.app.emit("config:changed", &new);
        let _ = self.controller.send(Msg::ConfigChanged);
        Ok(())
    }

    /// Profile id that should be active for the given foreground exe.
    pub fn profile_id_for(&self, exe: Option<&str>) -> String {
        if let Some(id) = self.manual_profile.lock().clone() {
            if self.profiles.read().get(&id).is_some() {
                return id;
            }
        }
        self.profiles.read().select_for_exe(exe).id.clone()
    }

    pub fn record_latency(&self, ms: f64) {
        let mut l = self.latencies.lock();
        if l.len() >= 50 {
            l.pop_front();
        }
        l.push_back(ms);
    }

    pub fn notify_profiles_changed(&self) {
        let _ = self.app.emit("profiles:changed", ());
        let _ = self.controller.send(Msg::ProfilesChanged);
    }
}

pub fn toggle_paused(app: &AppHandle) {
    let Some(core) = try_core() else { return };
    let paused = !core.paused.fetch_xor(true, Ordering::SeqCst);
    SHARED.paused.store(paused, Ordering::SeqCst);
    tracing::info!(paused, "pause toggled");
    crate::tray::refresh(app, core.lang(), paused);
    if paused {
        let _ = core.controller.send(Msg::Close);
    }
    let _ = app.emit("app:paused", paused);
}

/// Applies the autostart setting to the registry Run key (SPEC 8: on by default).
/// Dev builds never register themselves.
pub fn sync_autostart(app: &AppHandle) {
    use tauri_plugin_autostart::ManagerExt;
    if cfg!(debug_assertions) {
        return;
    }
    let Some(core) = try_core() else { return };
    let want = core.config.read().autostart;
    let mgr = app.autolaunch();
    let have = mgr.is_enabled().unwrap_or(false);
    let result = match (want, have) {
        (true, false) => mgr.enable(),
        (false, true) => mgr.disable(),
        _ => Ok(()),
    };
    if let Err(e) = result {
        tracing::warn!("cannot update autostart: {e}");
    }
}
