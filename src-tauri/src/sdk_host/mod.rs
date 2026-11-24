//! Host for Stream Deck plugins (SPEC 7): local WebSocket server, plugin processes,
//! visibility of button instances, events in both directions, settings and the
//! Property Inspector. Implemented from the public SDK documentation.

pub mod install;
pub mod manifest;
pub mod messages;
pub mod protocol;
pub mod settings;

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context};
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use serde::Serialize;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use crate::profiles::model::{Action, Button, Icon, Visual};
use crate::state::DynamicVisual;
use manifest::{Manifest, Runtime};
use messages::{Incoming, Instance};
use settings::SettingsStore;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub enum PluginStatus {
    Starting,
    Running,
    Stopped,
    /// Crashed too often or could not start.
    Error(String),
    /// No Windows code / unsupported runtime.
    Incompatible(String),
    /// Needs Node.js which is not installed.
    NodeMissing,
}

struct PluginEntry {
    manifest: Manifest,
    dir: PathBuf,
    status: PluginStatus,
    child: Option<Arc<Mutex<Child>>>,
    /// Incremented on every (re)start; a monitor thread only acts for its generation.
    generation: u64,
    restarts: VecDeque<Instant>,
    html_window: Option<String>,
}

struct PiConn {
    plugin: String,
    action: String,
    context: String,
    tx: Option<mpsc::UnboundedSender<String>>,
}

#[derive(Debug, Clone, Default)]
struct Runtime2 {
    title: Option<String>,
    image: Option<String>,
    state: u8,
}

#[derive(Default)]
struct Inner {
    plugins: BTreeMap<String, PluginEntry>,
    /// Live plugin sockets: uuid -> (connection id, sender). The id makes sure a closing old
    /// socket of a restarted plugin cannot remove the connection of the new process.
    connections: HashMap<String, (u64, mpsc::UnboundedSender<String>)>,
    pis: HashMap<String, PiConn>,
    /// Appeared instances: context -> (plugin uuid, instance).
    visible: BTreeMap<String, (String, Instance)>,
    runtime: HashMap<String, Runtime2>,
    device: (u8, u8),
    /// Running processes watched for ApplicationsToMonitor (lower-case exe).
    running_apps: HashSet<String>,
}

pub struct Host {
    plugins_dir: PathBuf,
    logs_dir: PathBuf,
    settings: SettingsStore,
    port: u16,
    app: Option<tauri::AppHandle>,
    inner: Mutex<Inner>,
}

static HOST: OnceLock<Arc<Host>> = OnceLock::new();
static NEXT_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static NEXT_CONNECTION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub fn host() -> Option<&'static Arc<Host>> {
    HOST.get()
}

/// Starts the host for the app (plugins from `%APPDATA%\Deckless\plugins`).
pub fn start_global(app: tauri::AppHandle) -> anyhow::Result<()> {
    let host = Host::start(
        &crate::paths::plugins_dir(),
        &crate::paths::plugin_settings_dir(),
        &crate::paths::logs_dir(),
        Some(app),
    )?;
    let _ = HOST.set(host.clone());
    host.load_installed();
    refresh_visible();
    Ok(())
}

fn lang() -> &'static str {
    crate::app::try_core()
        .map(|c| c.lang().code())
        .unwrap_or("en")
}

/// Normalises images from setImage into a data URL usable in `<img src>`.
pub fn normalize_image(image: &str) -> String {
    let s = image.trim();
    if let Some(rest) = s.strip_prefix("data:image/svg+xml") {
        if !rest.contains(";base64,") {
            if let Some((_, body)) = rest.split_once(',') {
                let body = urlencoding_decode(body);
                return format!(
                    "data:image/svg+xml;base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(body.as_bytes())
                );
            }
        }
        return s.to_string();
    }
    if s.starts_with("data:") {
        return s.to_string();
    }
    if s.starts_with("<svg") || s.starts_with("<?xml") {
        return format!(
            "data:image/svg+xml;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(s.as_bytes())
        );
    }
    // Plain base64 PNG without a prefix.
    format!("data:image/png;base64,{s}")
}

fn urlencoding_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// Finds `node.exe` on PATH.
pub fn find_node() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|p| p.join("node.exe"))
        .find(|p| p.is_file())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionInfo {
    pub uuid: String,
    pub name: String,
    pub tooltip: String,
    pub icon: Option<String>,
    pub supported: bool,
    pub has_property_inspector: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInfo {
    pub uuid: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub icon: Option<String>,
    pub status: PluginStatus,
    pub actions: Vec<ActionInfo>,
}

/// Decoded plugin images by file path. Decoding and re-encoding PNGs is slow, and the
/// editor asks for the plugin list often, so each image is converted only once.
static IMAGE_CACHE: Mutex<Option<HashMap<PathBuf, Option<String>>>> = Mutex::new(None);

fn image_data_url(dir: &Path, rel: &str) -> Option<String> {
    let p = manifest::resolve_image(dir, rel)?;
    if let Some(hit) = IMAGE_CACHE.lock().as_ref().and_then(|c| c.get(&p).cloned()) {
        return hit;
    }
    let url = crate::icons::load_image_data_url(&p).ok();
    IMAGE_CACHE
        .lock()
        .get_or_insert_with(HashMap::new)
        .insert(p, url.clone());
    url
}

/// Forgets cached images of a plugin folder (after an update or uninstall).
fn forget_images(dir: &Path) {
    if let Some(c) = IMAGE_CACHE.lock().as_mut() {
        c.retain(|k, _| !k.starts_with(dir));
    }
}

impl Host {
    pub fn start(
        plugins_dir: &Path,
        settings_dir: &Path,
        logs_dir: &Path,
        app: Option<tauri::AppHandle>,
    ) -> anyhow::Result<Arc<Host>> {
        std::fs::create_dir_all(plugins_dir)?;
        let listener =
            tauri::async_runtime::block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))?;
        let port = listener.local_addr()?.port();
        let host = Arc::new(Host {
            plugins_dir: plugins_dir.to_path_buf(),
            logs_dir: logs_dir.to_path_buf(),
            settings: SettingsStore::new(settings_dir),
            port,
            app,
            inner: Mutex::new(Inner {
                device: (3, 3),
                ..Default::default()
            }),
        });
        let h = host.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((tcp, _)) => {
                        let h = h.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Err(e) = h.handle_connection(tcp).await {
                                tracing::debug!("plugin connection ended: {e:#}");
                            }
                        });
                    }
                    Err(e) => {
                        tracing::error!("plugin server accept failed: {e}");
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                }
            }
        });
        host.clone().start_app_monitor();
        tracing::info!(port, "plugin host listening");
        Ok(host)
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Loads and starts every installed plugin.
    pub fn load_installed(self: &Arc<Self>) {
        let Ok(entries) = std::fs::read_dir(&self.plugins_dir) else {
            return;
        };
        for e in entries.flatten() {
            let dir = e.path();
            if !dir.is_dir() || !dir.to_string_lossy().ends_with(".sdPlugin") {
                continue;
            }
            match Manifest::load(&dir) {
                Ok(m) => {
                    let uuid = m.uuid.clone();
                    self.register_entry(m, dir);
                    self.start_plugin(&uuid);
                }
                Err(e) => tracing::error!("cannot load plugin {}: {e:#}", dir.display()),
            }
        }
    }

    fn register_entry(&self, manifest: Manifest, dir: PathBuf) {
        let status = match manifest.runtime() {
            None => PluginStatus::Incompatible("no Windows code".into()),
            Some(Runtime::Node { .. }) if find_node().is_none() => PluginStatus::NodeMissing,
            Some(_) => PluginStatus::Stopped,
        };
        let uuid = manifest.uuid.clone();
        self.inner.lock().plugins.insert(
            uuid,
            PluginEntry {
                manifest,
                dir,
                status,
                child: None,
                generation: 0,
                restarts: VecDeque::new(),
                html_window: None,
            },
        );
    }

    fn set_status(&self, uuid: &str, status: PluginStatus) {
        if let Some(p) = self.inner.lock().plugins.get_mut(uuid) {
            p.status = status;
        }
        self.notify_editor();
    }

    fn notify_editor(&self) {
        if let Some(app) = &self.app {
            use tauri::Emitter;
            let _ = app.emit("plugins:changed", ());
        }
    }

    pub fn status(&self, uuid: &str) -> Option<PluginStatus> {
        self.inner
            .lock()
            .plugins
            .get(uuid)
            .map(|p| p.status.clone())
    }

    pub fn is_connected(&self, uuid: &str) -> bool {
        self.inner.lock().connections.contains_key(uuid)
    }

    /// Starts the plugin's code (node / exe / hidden webview).
    pub fn start_plugin(self: &Arc<Self>, uuid: &str) {
        let (runtime, dir, version, generation) = {
            let mut inner = self.inner.lock();
            let device = inner.device;
            let Some(p) = inner.plugins.get_mut(uuid) else {
                return;
            };
            if matches!(
                p.status,
                PluginStatus::Incompatible(_) | PluginStatus::NodeMissing
            ) {
                return;
            }
            // Globally unique, so a watcher of an older process (even of a reinstalled
            // plugin entry) can never mistake the new process for its own.
            p.generation = NEXT_GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            p.status = PluginStatus::Starting;
            let _ = device;
            (
                p.manifest.runtime(),
                p.dir.clone(),
                p.manifest.version.clone(),
                p.generation,
            )
        };
        let Some(runtime) = runtime else { return };
        let (cols, rows) = self.inner.lock().device;
        let info = messages::info_json(uuid, &version, lang(), cols, rows).to_string();
        let args = [
            "-port".to_string(),
            self.port.to_string(),
            "-pluginUUID".to_string(),
            uuid.to_string(),
            "-registerEvent".to_string(),
            "registerPlugin".to_string(),
            "-info".to_string(),
            info.clone(),
        ];
        let spawn = |program: PathBuf, prefix: Vec<String>| -> anyhow::Result<Child> {
            use std::os::windows::process::CommandExt;
            let log = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(self.logs_dir.join(format!("plugin-{uuid}.log")))?;
            let mut cmd = Command::new(program);
            cmd.args(prefix)
                .args(&args)
                .current_dir(&dir)
                .stdin(Stdio::null())
                .stdout(log.try_clone()?)
                .stderr(log)
                .creation_flags(windows::Win32::System::Threading::CREATE_NO_WINDOW.0);
            Ok(cmd.spawn()?)
        };
        let result = match &runtime {
            Runtime::Node { script } => match find_node() {
                Some(node) => spawn(node, vec![dir.join(script).to_string_lossy().to_string()]),
                None => {
                    self.set_status(uuid, PluginStatus::NodeMissing);
                    return;
                }
            },
            Runtime::Exe { exe } => spawn(dir.join(exe), vec![]),
            Runtime::Html { page } => {
                if let Err(e) = self.open_html_plugin(uuid, page, &info) {
                    self.set_status(uuid, PluginStatus::Error(format!("{e:#}")));
                }
                return;
            }
        };
        match result {
            Ok(child) => {
                tracing::info!(plugin = uuid, pid = child.id(), "plugin started");
                attach_to_job(&child);
                let child = Arc::new(Mutex::new(child));
                if let Some(p) = self.inner.lock().plugins.get_mut(uuid) {
                    p.child = Some(child.clone());
                }
                self.watch_process(uuid.to_string(), child, generation);
            }
            Err(e) => {
                tracing::error!(plugin = uuid, "cannot start plugin: {e:#}");
                self.set_status(uuid, PluginStatus::Error(format!("{e:#}")));
            }
        }
    }

    fn open_html_plugin(&self, uuid: &str, page: &str, info: &str) -> anyhow::Result<()> {
        let app = self
            .app
            .as_ref()
            .ok_or_else(|| anyhow!("no window system"))?;
        let label = format!(
            "plugin-{}",
            uuid.replace(|c: char| !c.is_ascii_alphanumeric(), "-")
        );
        let connect = json!({
            "port": self.port, "uuid": uuid, "event": "registerPlugin", "info": info,
        });
        let url = protocol::page_url(uuid, page, &connect);
        tauri::WebviewWindowBuilder::new(app, &label, tauri::WebviewUrl::External(url.parse()?))
            .visible(false)
            .skip_taskbar(true)
            .build()?;
        if let Some(p) = self.inner.lock().plugins.get_mut(uuid) {
            p.html_window = Some(label);
        }
        Ok(())
    }

    /// Restarts a crashed plugin at most 3 times per minute (SPEC 7.2).
    fn watch_process(self: &Arc<Self>, uuid: String, child: Arc<Mutex<Child>>, generation: u64) {
        let host = self.clone();
        std::thread::Builder::new()
            .name(format!("plugin-{uuid}"))
            .spawn(move || {
                let status = loop {
                    if let Ok(Some(s)) = child.lock().try_wait() {
                        break s;
                    }
                    std::thread::sleep(Duration::from_millis(250));
                };
                let restart = {
                    let mut inner = host.inner.lock();
                    let Some(p) = inner.plugins.get_mut(&uuid) else {
                        return;
                    };
                    if p.generation != generation || p.status == PluginStatus::Stopped {
                        return; // stopped on purpose or already restarted
                    }
                    p.child = None;
                    let now = Instant::now();
                    while p
                        .restarts
                        .front()
                        .is_some_and(|t| now.duration_since(*t) > Duration::from_secs(60))
                    {
                        p.restarts.pop_front();
                    }
                    if p.restarts.len() >= 3 {
                        p.status = PluginStatus::Error(format!("crashed repeatedly ({status})"));
                        false
                    } else {
                        p.restarts.push_back(now);
                        true
                    }
                };
                tracing::warn!(plugin = %uuid, %status, restart, "plugin exited");
                host.notify_editor();
                if restart {
                    std::thread::sleep(Duration::from_millis(500));
                    host.start_plugin(&uuid);
                }
            })
            .ok();
    }

    pub fn stop_plugin(&self, uuid: &str) {
        let (child, window) = {
            let mut inner = self.inner.lock();
            inner.connections.remove(uuid);
            let Some(p) = inner.plugins.get_mut(uuid) else {
                return;
            };
            p.status = PluginStatus::Stopped;
            (p.child.take(), p.html_window.take())
        };
        if let Some(c) = child {
            let mut c = c.lock();
            let _ = c.kill();
            let _ = c.wait();
        }
        if let (Some(label), Some(app)) = (window, &self.app) {
            use tauri::Manager;
            if let Some(w) = app.get_webview_window(&label) {
                let _ = w.destroy();
            }
        }
    }

    pub fn stop_all(&self) {
        let uuids: Vec<String> = self.inner.lock().plugins.keys().cloned().collect();
        for u in uuids {
            self.stop_plugin(&u);
        }
    }

    /// On app exit: terminate plugin processes without waiting for them. (They are in a
    /// kill-on-close job object anyway, so nothing can outlive the app.)
    pub fn kill_all_now(&self) {
        let children: Vec<Arc<Mutex<Child>>> = {
            let mut inner = self.inner.lock();
            inner.connections.clear();
            inner
                .plugins
                .values_mut()
                .filter_map(|p| {
                    p.status = PluginStatus::Stopped;
                    p.child.take()
                })
                .collect()
        };
        for c in children {
            if let Some(mut c) = c.try_lock() {
                let _ = c.kill();
            }
        }
    }

    /// Installs (or updates) a plugin from a `.streamDeckPlugin` file or `.sdPlugin` folder.
    pub fn install(self: &Arc<Self>, source: &Path) -> anyhow::Result<PluginInfo> {
        let m = install::install(source, &self.plugins_dir, |uuid| self.stop_plugin(uuid))?;
        let uuid = m.uuid.clone();
        let dir = self.plugins_dir.join(format!("{uuid}.sdPlugin"));
        forget_images(&dir);
        self.register_entry(m, dir);
        self.start_plugin(&uuid);
        self.notify_editor();
        tracing::info!(plugin = %uuid, "plugin installed");
        self.plugin_info(&uuid)
            .ok_or_else(|| anyhow!("plugin disappeared"))
    }

    pub fn uninstall(&self, uuid: &str) -> anyhow::Result<()> {
        self.stop_plugin(uuid);
        let dir = self
            .inner
            .lock()
            .plugins
            .remove(uuid)
            .map(|p| p.dir)
            .ok_or_else(|| anyhow!("plugin not installed"))?;
        // The process may need a moment to release its files.
        let mut last = None;
        for _ in 0..10 {
            match std::fs::remove_dir_all(&dir) {
                Ok(()) => {
                    last = None;
                    break;
                }
                Err(e) => {
                    last = Some(e);
                    std::thread::sleep(Duration::from_millis(200));
                }
            }
        }
        if let Some(e) = last {
            return Err(e).context("cannot delete plugin files");
        }
        forget_images(&dir);
        self.settings.remove_plugin(uuid);
        self.notify_editor();
        Ok(())
    }

    pub fn plugin_info(&self, uuid: &str) -> Option<PluginInfo> {
        let inner = self.inner.lock();
        let p = inner.plugins.get(uuid)?;
        Some(PluginInfo {
            uuid: uuid.to_string(),
            name: p.manifest.name.clone(),
            version: p.manifest.version.clone(),
            author: p.manifest.author.clone(),
            description: p.manifest.description.clone(),
            icon: image_data_url(&p.dir, &p.manifest.icon),
            status: p.status.clone(),
            actions: p
                .manifest
                .actions
                .iter()
                .filter(|a| a.visible_in_action_list != Some(false))
                .map(|a| ActionInfo {
                    uuid: a.uuid.clone(),
                    name: a.name.clone(),
                    tooltip: a.tooltip.clone(),
                    icon: image_data_url(&p.dir, &a.icon),
                    supported: a.is_keypad(),
                    has_property_inspector: a.property_inspector_path.is_some()
                        || p.manifest.property_inspector_path.is_some(),
                })
                .collect(),
        })
    }

    /// Installed plugins and their versions (cheap: no images).
    pub fn versions(&self) -> HashMap<String, String> {
        self.inner
            .lock()
            .plugins
            .iter()
            .map(|(u, p)| (u.clone(), p.manifest.version.clone()))
            .collect()
    }

    pub fn list(&self) -> Vec<PluginInfo> {
        let uuids: Vec<String> = self.inner.lock().plugins.keys().cloned().collect();
        uuids.iter().filter_map(|u| self.plugin_info(u)).collect()
    }

    /// Default title, icon and second-state look for a new button of a plugin action.
    pub fn action_defaults(
        &self,
        plugin: &str,
        action: &str,
    ) -> Option<(String, Option<Icon>, Option<Visual>)> {
        let inner = self.inner.lock();
        let p = inner.plugins.get(plugin)?;
        let a = p.manifest.action(action)?;
        let icon_of = |i: usize| -> Option<Icon> {
            let st = a.states.get(i)?;
            let rel = if st.image.is_empty() {
                &a.icon
            } else {
                &st.image
            };
            image_data_url(&p.dir, rel).map(|src| Icon::Image { src })
        };
        let title = a
            .states
            .first()
            .map(|s| s.title.clone())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| a.name.clone());
        let icon =
            icon_of(0).or_else(|| image_data_url(&p.dir, &a.icon).map(|src| Icon::Image { src }));
        let state_b = (a.states.len() > 1).then(|| Visual {
            title: a.states[1].title.clone(),
            icon: icon_of(1),
        });
        Some((title, icon, state_b))
    }

    // ---- visibility and key presses -------------------------------------------------

    fn send_to(&self, plugin: &str, msg: &Value) {
        if let Some((_, tx)) = self.inner.lock().connections.get(plugin) {
            let _ = tx.send(msg.to_string());
        }
    }

    /// Sets the appeared instances: sends willDisappear for removed and willAppear for new.
    pub fn set_visible(&self, list: Vec<(String, Instance)>, device: (u8, u8)) {
        let (removed, added, device_changed) = {
            let mut inner = self.inner.lock();
            let new: BTreeMap<String, (String, Instance)> = list
                .into_iter()
                .map(|(p, i)| (i.context.clone(), (p, i)))
                .collect();
            let removed: Vec<(String, Instance)> = inner
                .visible
                .iter()
                .filter(|(c, (p, i))| match new.get(*c) {
                    Some((np, ni)) => {
                        np != p || ni.column != i.column || ni.row != i.row || ni.action != i.action
                    }
                    None => true,
                })
                .map(|(_, v)| v.clone())
                .collect();
            let added: Vec<(String, Instance)> = new
                .iter()
                .filter(|(c, v)| {
                    inner.visible.get(*c) != Some(*v)
                        || removed.iter().any(|(_, r)| &r.context == *c)
                })
                .map(|(_, v)| v.clone())
                .collect();
            let device_changed = inner.device != device;
            inner.device = device;
            inner.visible = new;
            (removed, added, device_changed)
        };
        if device_changed {
            let plugins: Vec<String> = self.inner.lock().connections.keys().cloned().collect();
            for p in plugins {
                self.send_to(&p, &messages::device_did_connect(device.0, device.1));
            }
        }
        for (p, mut i) in removed {
            i.settings = self.settings.button(&p, &i.context);
            self.send_to(&p, &messages::instance_event("willDisappear", &i));
        }
        for (p, mut i) in added {
            i.settings = self.settings.button(&p, &i.context);
            i.state = self.state_of(&i.context);
            self.send_to(&p, &messages::instance_event("willAppear", &i));
        }
    }

    fn state_of(&self, context: &str) -> u8 {
        self.inner
            .lock()
            .runtime
            .get(context)
            .map(|r| r.state)
            .unwrap_or(0)
    }

    /// Selecting the button in the panel = keyDown followed by keyUp (SPEC 7.3).
    pub fn press(
        &self,
        plugin: &str,
        action: &str,
        context: &str,
        coords: (u8, u8),
    ) -> anyhow::Result<()> {
        if !self.is_connected(plugin) {
            let status = self.status(plugin);
            bail!(match status {
                None => "the plugin is not installed".to_string(),
                Some(PluginStatus::NodeMissing) =>
                    "the plugin needs Node.js (nodejs.org)".to_string(),
                Some(PluginStatus::Running | PluginStatus::Starting) =>
                    "the plugin is just starting; try again in a moment".to_string(),
                Some(PluginStatus::Error(e)) => format!("the plugin stopped working: {e}"),
                Some(PluginStatus::Incompatible(e)) => format!("the plugin cannot run here: {e}"),
                Some(PluginStatus::Stopped) => "the plugin is not running".to_string(),
            });
        }
        let visible = self.inner.lock().visible.contains_key(context);
        let mut inst = Instance {
            action: action.to_string(),
            context: context.to_string(),
            column: coords.0,
            row: coords.1,
            state: self.state_of(context),
            settings: self.settings.button(plugin, context),
        };
        if !visible {
            // Not on the current page (e.g. "Try it" in the editor): make it appear first.
            self.send_to(plugin, &messages::instance_event("willAppear", &inst));
            self.inner
                .lock()
                .visible
                .insert(context.to_string(), (plugin.to_string(), inst.clone()));
        }
        self.send_to(plugin, &messages::instance_event("keyDown", &inst));
        inst.settings = self.settings.button(plugin, context);
        self.send_to(plugin, &messages::instance_event("keyUp", &inst));
        // Two-state actions switch state automatically unless the manifest disables it.
        let auto = {
            let inner = self.inner.lock();
            inner
                .plugins
                .get(plugin)
                .and_then(|p| p.manifest.action(action))
                .is_some_and(|a| a.states.len() == 2 && !a.disable_automatic_states)
        };
        if auto {
            let mut inner = self.inner.lock();
            let r = inner.runtime.entry(context.to_string()).or_default();
            r.state = 1 - r.state.min(1);
        }
        Ok(())
    }

    pub fn runtime_visual(&self, context: &str) -> Option<(Option<String>, Option<String>, u8)> {
        self.inner
            .lock()
            .runtime
            .get(context)
            .map(|r| (r.title.clone(), r.image.clone(), r.state))
    }

    pub fn button_settings(&self, plugin: &str, context: &str) -> Value {
        self.settings.button(plugin, context)
    }

    // ---- Property Inspector ---------------------------------------------------------

    /// URL of the Property Inspector page for a button (loaded in an iframe in the editor).
    pub fn property_inspector_url(
        &self,
        plugin: &str,
        action: &str,
        context: &str,
    ) -> anyhow::Result<Option<String>> {
        let (path, version) = {
            let inner = self.inner.lock();
            let p = inner
                .plugins
                .get(plugin)
                .ok_or_else(|| anyhow!("plugin not installed"))?;
            let a = p.manifest.action(action);
            let path = a
                .and_then(|a| a.property_inspector_path.clone())
                .or_else(|| p.manifest.property_inspector_path.clone());
            (path, p.manifest.version.clone())
        };
        let Some(path) = path else { return Ok(None) };
        let pi_uuid = format!("pi-{}", uuid::Uuid::new_v4().simple());
        let (cols, rows) = self.inner.lock().device;
        let coords = self
            .inner
            .lock()
            .visible
            .get(context)
            .map(|(_, i)| (i.column, i.row))
            .unwrap_or((0, 0));
        let action_info = json!({
            "action": action,
            "context": context,
            "device": messages::DEVICE_ID,
            "payload": {
                "settings": self.settings.button(plugin, context),
                "coordinates": { "column": coords.0, "row": coords.1 },
                "controller": "Keypad",
            },
        });
        let info = messages::info_json(plugin, &version, lang(), cols, rows);
        let connect = json!({
            "port": self.port,
            "uuid": pi_uuid,
            "event": "registerPropertyInspector",
            "info": info.to_string(),
            "actionInfo": action_info.to_string(),
        });
        let mut inner = self.inner.lock();
        // Inspectors that were opened for this button but never connected are stale.
        inner
            .pis
            .retain(|_, p| p.tx.is_some() || p.context != context);
        inner.pis.insert(
            pi_uuid,
            PiConn {
                plugin: plugin.to_string(),
                action: action.to_string(),
                context: context.to_string(),
                tx: None,
            },
        );
        Ok(Some(protocol::page_url(plugin, &path, &connect)))
    }

    // ---- WebSocket ------------------------------------------------------------------

    async fn handle_connection(self: Arc<Self>, tcp: tokio::net::TcpStream) -> anyhow::Result<()> {
        let ws = tokio_tungstenite::accept_async(tcp).await?;
        let (mut sink, mut stream) = ws.split();
        let first = loop {
            match stream.next().await {
                Some(Ok(Message::Text(t))) => break t.to_string(),
                Some(Ok(_)) => continue,
                _ => bail!("closed before registering"),
            }
        };
        let (tx, mut rx) = mpsc::unbounded_channel::<String>();
        tauri::async_runtime::spawn(async move {
            while let Some(m) = rx.recv().await {
                if sink.send(Message::Text(m.into())).await.is_err() {
                    break;
                }
            }
            let _ = sink.close().await;
        });
        enum Kind {
            Plugin(String, u64),
            Pi(String),
        }
        let kind = match messages::parse(&first)? {
            Incoming::RegisterPlugin { uuid } => {
                let id = self.on_plugin_registered(&uuid, tx);
                Kind::Plugin(uuid, id)
            }
            Incoming::RegisterPropertyInspector { uuid } => {
                self.on_pi_registered(&uuid, tx);
                Kind::Pi(uuid)
            }
            other => bail!("expected a register event, got {other:?}"),
        };
        while let Some(msg) = stream.next().await {
            let Ok(Message::Text(text)) = msg else {
                if matches!(msg, Ok(Message::Close(_)) | Err(_)) {
                    break;
                }
                continue;
            };
            let parsed = match messages::parse(&text) {
                Ok(p) => p,
                Err(e) => {
                    tracing::debug!("invalid plugin message: {e}");
                    continue;
                }
            };
            match &kind {
                Kind::Plugin(uuid, _) => self.on_plugin_message(uuid, parsed),
                Kind::Pi(uuid) => self.on_pi_message(uuid, parsed),
            }
        }
        match kind {
            Kind::Plugin(uuid, id) => {
                let mut inner = self.inner.lock();
                if inner
                    .connections
                    .get(&uuid)
                    .is_some_and(|(cid, _)| *cid == id)
                {
                    inner.connections.remove(&uuid);
                }
            }
            Kind::Pi(uuid) => {
                if let Some(pi) = self.inner.lock().pis.remove(&uuid) {
                    self.send_to(
                        &pi.plugin,
                        &messages::pi_event(
                            "propertyInspectorDidDisappear",
                            &pi.action,
                            &pi.context,
                        ),
                    );
                }
            }
        }
        Ok(())
    }

    fn on_plugin_registered(&self, uuid: &str, tx: mpsc::UnboundedSender<String>) -> u64 {
        tracing::info!(plugin = uuid, "plugin registered");
        let conn_id = NEXT_CONNECTION.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let (device, visible, monitored) = {
            let mut inner = self.inner.lock();
            inner.connections.insert(uuid.to_string(), (conn_id, tx));
            if let Some(p) = inner.plugins.get_mut(uuid) {
                p.status = PluginStatus::Running;
            }
            let visible: Vec<Instance> = inner
                .visible
                .values()
                .filter(|(p, _)| p == uuid)
                .map(|(_, i)| i.clone())
                .collect();
            let monitored: Vec<String> = inner
                .plugins
                .get(uuid)
                .and_then(|p| p.manifest.applications_to_monitor.clone())
                .map(|a| a.windows)
                .unwrap_or_default()
                .into_iter()
                .filter(|app| inner.running_apps.contains(&app.to_lowercase()))
                .collect();
            (inner.device, visible, monitored)
        };
        self.send_to(uuid, &messages::device_did_connect(device.0, device.1));
        for mut i in visible {
            i.settings = self.settings.button(uuid, &i.context);
            i.state = self.state_of(&i.context);
            self.send_to(uuid, &messages::instance_event("willAppear", &i));
        }
        for app in monitored {
            self.send_to(uuid, &messages::application_event(true, &app));
        }
        self.notify_editor();
        conn_id
    }

    fn instance_for(&self, plugin: &str, context: &str) -> Instance {
        let inner = self.inner.lock();
        let (action, column, row) = inner
            .visible
            .get(context)
            .map(|(_, i)| (i.action.clone(), i.column, i.row))
            .or_else(|| {
                inner
                    .pis
                    .values()
                    .find(|p| p.context == context)
                    .map(|p| (p.action.clone(), 0, 0))
            })
            .unwrap_or_default();
        drop(inner);
        Instance {
            action,
            context: context.to_string(),
            column,
            row,
            state: self.state_of(context),
            settings: self.settings.button(plugin, context),
        }
    }

    fn pis_for_context(&self, context: &str) -> Vec<mpsc::UnboundedSender<String>> {
        self.inner
            .lock()
            .pis
            .values()
            .filter(|p| p.context == context)
            .filter_map(|p| p.tx.clone())
            .collect()
    }

    fn update_runtime(&self, context: &str, f: impl FnOnce(&mut Runtime2)) {
        f(self
            .inner
            .lock()
            .runtime
            .entry(context.to_string())
            .or_default());
        if let Some(app) = &self.app {
            use tauri::Emitter;
            let _ = app.emit("plugins:visual", context);
        }
    }

    fn on_plugin_message(&self, plugin: &str, msg: Incoming) {
        match msg {
            Incoming::SetTitle { context, title, .. } => {
                self.update_runtime(&context, |r| r.title = title.filter(|t| !t.is_empty()));
            }
            Incoming::SetImage { context, image, .. } => {
                self.update_runtime(&context, |r| r.image = image.map(|i| normalize_image(&i)));
            }
            Incoming::SetState { context, state } => {
                self.update_runtime(&context, |r| r.state = state)
            }
            Incoming::ShowAlert { context } => {
                if let Some(core) = crate::app::try_core() {
                    let _ = core
                        .controller
                        .send(crate::overlay::controller::Msg::ActionFailed {
                            button_id: context,
                            hint: None,
                        });
                }
            }
            Incoming::ShowOk { context } => tracing::debug!(plugin, context, "showOk"),
            Incoming::SetSettings { context, settings } => {
                self.settings.set_button(plugin, &context, settings);
                let inst = self.instance_for(plugin, &context);
                for tx in self.pis_for_context(&context) {
                    let _ =
                        tx.send(messages::instance_event("didReceiveSettings", &inst).to_string());
                }
            }
            Incoming::GetSettings { context } => {
                let inst = self.instance_for(plugin, &context);
                self.send_to(
                    plugin,
                    &messages::instance_event("didReceiveSettings", &inst),
                );
            }
            Incoming::SetGlobalSettings { settings, .. } => {
                self.settings.set_global(plugin, settings.clone());
                let pis: Vec<_> = self
                    .inner
                    .lock()
                    .pis
                    .values()
                    .filter(|p| p.plugin == plugin)
                    .filter_map(|p| p.tx.clone())
                    .collect();
                for tx in pis {
                    let _ = tx.send(messages::global_settings(&settings).to_string());
                }
            }
            Incoming::GetGlobalSettings { .. } => {
                self.send_to(
                    plugin,
                    &messages::global_settings(&self.settings.global(plugin)),
                );
            }
            Incoming::OpenUrl { url } => open_url(&url),
            Incoming::LogMessage { message } => tracing::info!(plugin, "plugin log: {message}"),
            Incoming::SwitchToProfile { profile } => {
                tracing::info!(
                    plugin,
                    profile,
                    "switchToProfile ignored (bundled Stream Deck profiles are not supported)"
                );
            }
            Incoming::SendToPropertyInspector {
                action,
                context,
                payload,
            } => {
                for tx in self.pis_for_context(&context) {
                    let _ = tx.send(messages::send_to_pi(&action, &context, &payload).to_string());
                }
            }
            Incoming::SendToPlugin { .. }
            | Incoming::RegisterPlugin { .. }
            | Incoming::RegisterPropertyInspector { .. } => {}
            Incoming::Unknown { event } => tracing::debug!(plugin, event, "unknown plugin event"),
        }
    }

    fn on_pi_registered(&self, pi_uuid: &str, tx: mpsc::UnboundedSender<String>) {
        let pi = {
            let mut inner = self.inner.lock();
            let Some(pi) = inner.pis.get_mut(pi_uuid) else {
                tracing::warn!(pi_uuid, "unknown property inspector registered");
                return;
            };
            pi.tx = Some(tx);
            (pi.plugin.clone(), pi.action.clone(), pi.context.clone())
        };
        self.send_to(
            &pi.0,
            &messages::pi_event("propertyInspectorDidAppear", &pi.1, &pi.2),
        );
    }

    fn on_pi_message(&self, pi_uuid: &str, msg: Incoming) {
        let Some((plugin, action, context, tx)) = self.inner.lock().pis.get(pi_uuid).map(|p| {
            (
                p.plugin.clone(),
                p.action.clone(),
                p.context.clone(),
                p.tx.clone(),
            )
        }) else {
            return;
        };
        match msg {
            Incoming::SetSettings { settings, .. } => {
                self.settings.set_button(&plugin, &context, settings);
                let inst = self.instance_for(&plugin, &context);
                self.send_to(
                    &plugin,
                    &messages::instance_event("didReceiveSettings", &inst),
                );
            }
            Incoming::GetSettings { .. } => {
                let inst = self.instance_for(&plugin, &context);
                if let Some(tx) = tx {
                    let _ =
                        tx.send(messages::instance_event("didReceiveSettings", &inst).to_string());
                }
            }
            Incoming::SetGlobalSettings { settings, .. } => {
                self.settings.set_global(&plugin, settings.clone());
                self.send_to(&plugin, &messages::global_settings(&settings));
            }
            Incoming::GetGlobalSettings { .. } => {
                if let Some(tx) = tx {
                    let _ = tx.send(
                        messages::global_settings(&self.settings.global(&plugin)).to_string(),
                    );
                }
            }
            Incoming::SendToPlugin { payload, .. } => {
                self.send_to(
                    &plugin,
                    &messages::send_to_plugin(&action, &context, &payload),
                );
            }
            Incoming::OpenUrl { url } => open_url(&url),
            Incoming::LogMessage { message } => {
                tracing::info!(plugin, "property inspector log: {message}")
            }
            _ => {}
        }
    }

    // ---- ApplicationsToMonitor and wake-up ---------------------------------------------

    fn start_app_monitor(self: Arc<Self>) {
        std::thread::Builder::new()
            .name("plugin-app-monitor".into())
            .spawn(move || {
                let mut last_tick = std::time::SystemTime::now();
                loop {
                    std::thread::sleep(Duration::from_secs(2));
                    // A jump in wall-clock time means the PC was asleep.
                    let now = std::time::SystemTime::now();
                    if now.duration_since(last_tick).unwrap_or_default() > Duration::from_secs(30) {
                        let plugins: Vec<String> =
                            self.inner.lock().connections.keys().cloned().collect();
                        for p in plugins {
                            self.send_to(&p, &messages::system_did_wake_up());
                        }
                    }
                    last_tick = now;

                    let watched: Vec<(String, Vec<String>)> = {
                        let inner = self.inner.lock();
                        inner
                            .plugins
                            .iter()
                            .filter(|(u, _)| inner.connections.contains_key(*u))
                            .filter_map(|(u, p)| {
                                p.manifest
                                    .applications_to_monitor
                                    .as_ref()
                                    .map(|a| (u.clone(), a.windows.clone()))
                            })
                            .filter(|(_, apps)| !apps.is_empty())
                            .collect()
                    };
                    if watched.is_empty() {
                        continue;
                    }
                    let running = running_process_names();
                    let before =
                        std::mem::replace(&mut self.inner.lock().running_apps, running.clone());
                    for (plugin, apps) in watched {
                        for app in apps {
                            let key = app.to_lowercase();
                            match (before.contains(&key), running.contains(&key)) {
                                (false, true) => {
                                    self.send_to(&plugin, &messages::application_event(true, &app))
                                }
                                (true, false) => {
                                    self.send_to(&plugin, &messages::application_event(false, &app))
                                }
                                _ => {}
                            }
                        }
                    }
                }
            })
            .ok();
    }
}

fn open_url(url: &str) {
    let lower = url.to_lowercase();
    if lower.starts_with("https://") || lower.starts_with("http://") {
        if let Err(e) = crate::actions::shell::open(url) {
            tracing::warn!("cannot open {url}: {e:#}");
        }
    } else {
        tracing::warn!("plugin tried to open a non-web URL: {url}");
    }
}

/// Lower-case exe names of all running processes.
pub fn running_process_names() -> HashSet<String> {
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    let mut out = HashSet::new();
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return out;
        };
        let mut e = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snap, &mut e).is_ok() {
            loop {
                let len = e
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(e.szExeFile.len());
                out.insert(String::from_utf16_lossy(&e.szExeFile[..len]).to_lowercase());
                if Process32NextW(snap, &mut e).is_err() {
                    break;
                }
            }
        }
        let _ = windows::Win32::Foundation::CloseHandle(snap);
    }
    out
}

// ---- Glue used by the rest of the app ---------------------------------------------------

/// Recomputes which plugin buttons are visible (active profile + page shown in the panel).
pub fn refresh_visible() {
    let Some(host) = host() else { return };
    let Some(core) = crate::app::try_core() else {
        return;
    };
    let profile_id = crate::profiles::active::active_profile_id();
    let path = PAGE_PATH.lock().clone();
    let (layout, page) = {
        let store = core.profiles.read();
        let Some(profile) = store
            .get(&profile_id)
            .or_else(|| Some(store.default_profile()))
        else {
            return;
        };
        let page = store
            .effective_page(profile, &path)
            .or_else(|| store.effective_page(profile, &[]))
            .unwrap_or_default();
        (profile.layout, page)
    };
    let mut list = Vec::new();
    for (i, slot) in page.slots.iter().enumerate() {
        if let Some(Button {
            id,
            action:
                Action::Plugin {
                    plugin_uuid,
                    action_uuid,
                },
            ..
        }) = slot
        {
            let (column, row) = layout.coordinates(i);
            list.push((
                plugin_uuid.clone(),
                Instance {
                    action: action_uuid.clone(),
                    context: id.clone(),
                    column,
                    row,
                    state: 0,
                    settings: Value::Null,
                },
            ));
        }
    }
    host.set_visible(list, layout.device_size());
}

static PAGE_PATH: Mutex<Vec<usize>> = Mutex::new(Vec::new());

/// Called by the overlay when it shows another folder page (empty path = top page).
pub fn set_page_path(path: &[usize]) {
    let changed = {
        let mut p = PAGE_PATH.lock();
        if p.as_slice() == path {
            false
        } else {
            *p = path.to_vec();
            true
        }
    };
    if changed {
        refresh_visible();
    }
}

pub fn on_active_profile_changed() {
    PAGE_PATH.lock().clear();
    refresh_visible();
}

pub fn key_press(button: &Button) -> anyhow::Result<()> {
    let Action::Plugin {
        plugin_uuid,
        action_uuid,
    } = &button.action
    else {
        bail!("not a plugin action");
    };
    let host = host().ok_or_else(|| anyhow!("the plugin host is not running"))?;
    // Coordinates of the button in the active profile, if it is there.
    let coords = crate::app::try_core()
        .and_then(|core| {
            let store = core.profiles.read();
            let profile = store.get(&crate::profiles::active::active_profile_id())?;
            let page = store.effective_page(profile, &PAGE_PATH.lock())?;
            let i = page
                .slots
                .iter()
                .position(|s| s.as_ref().is_some_and(|b| b.id == button.id))?;
            Some(profile.layout.coordinates(i))
        })
        .unwrap_or((0, 0));
    host.press(plugin_uuid, action_uuid, &button.id, coords)
}

pub fn button_state(context: &str) -> u8 {
    host()
        .and_then(|h| h.runtime_visual(context))
        .map(|(_, _, s)| s)
        .unwrap_or(0)
}

pub fn button_visual(context: &str) -> Option<DynamicVisual> {
    let (title, image, _) = host()?.runtime_visual(context)?;
    if title.is_none() && image.is_none() {
        return None;
    }
    Some(DynamicVisual {
        title,
        icon: image.map(|src| Icon::Image { src }),
    })
}

/// Job object that kills plugin processes when Deckless exits or crashes.
fn plugin_job() -> Option<windows::Win32::Foundation::HANDLE> {
    use windows::Win32::System::JobObjects::{
        CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    static JOB: OnceLock<Option<isize>> = OnceLock::new();
    let h = JOB.get_or_init(|| unsafe {
        let job = CreateJobObjectW(None, None).ok()?;
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const _,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
        .ok()?;
        Some(job.0 as isize)
    });
    h.map(|h| windows::Win32::Foundation::HANDLE(h as *mut _))
}

fn attach_to_job(child: &Child) {
    use std::os::windows::io::AsRawHandle;
    if let Some(job) = plugin_job() {
        let process = windows::Win32::Foundation::HANDLE(child.as_raw_handle());
        unsafe {
            let _ = windows::Win32::System::JobObjects::AssignProcessToJobObject(job, process);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_images() {
        assert!(normalize_image("data:image/png;base64,AAA").starts_with("data:image/png;base64,"));
        let svg = normalize_image("data:image/svg+xml;charset=utf8,<svg xmlns='x'>%23</svg>");
        assert!(svg.starts_with("data:image/svg+xml;base64,"));
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(svg.split(',').nth(1).unwrap())
            .unwrap();
        assert_eq!(
            String::from_utf8(decoded).unwrap(),
            "<svg xmlns='x'>#</svg>"
        );
        assert!(normalize_image("<svg></svg>").starts_with("data:image/svg+xml;base64,"));
        assert_eq!(normalize_image("QUJD"), "data:image/png;base64,QUJD");
    }
}
