//! obs-websocket v5 client (SPEC 6). Connects lazily, reconnects with backoff, and keeps
//! the recording / streaming / mute state for live button states.

pub mod protocol;

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{anyhow, bail, Context};
use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;

use crate::app::try_core;
use crate::config::ObsConfig;
use crate::profiles::model::{Action, ObsOp};
use protocol::{auth_string, OpCode};

#[derive(Debug, Clone, Default)]
pub struct ObsStatus {
    pub connected: bool,
    pub recording: bool,
    pub streaming: bool,
    pub muted_sources: HashSet<String>,
}

static STATUS: Mutex<Option<ObsStatus>> = Mutex::new(None);
static STARTED: AtomicBool = AtomicBool::new(false);
static REQUESTS: Mutex<Option<mpsc::UnboundedSender<Req>>> = Mutex::new(None);

struct Req {
    kind: String,
    data: Value,
    reply: oneshot::Sender<anyhow::Result<Value>>,
}

pub fn status() -> ObsStatus {
    STATUS.lock().clone().unwrap_or_default()
}

fn set_status(f: impl FnOnce(&mut ObsStatus)) {
    let mut s = STATUS.lock();
    let st = s.get_or_insert_with(ObsStatus::default);
    f(st);
}

static CONFIG_OVERRIDE: Mutex<Option<ObsConfig>> = Mutex::new(None);

/// Uses these settings instead of the app config (integration tests run without the app).
pub fn set_config_override(cfg: Option<ObsConfig>) {
    *CONFIG_OVERRIDE.lock() = cfg;
}

fn config() -> Option<ObsConfig> {
    if let Some(c) = CONFIG_OVERRIDE.lock().clone() {
        return Some(c);
    }
    try_core().map(|c| c.config.read().obs.clone())
}

/// True if any profile has an OBS button (then we connect at startup for live state).
pub fn profiles_use_obs() -> bool {
    let Some(core) = try_core() else { return false };
    let store = core.profiles.read();
    store.list().iter().any(|p| {
        let mut found = false;
        p.for_each_button(&mut |b| {
            if uses_obs(&b.action) {
                found = true;
            }
        });
        found
    })
}

fn uses_obs(a: &Action) -> bool {
    match a {
        Action::Obs { .. } => true,
        Action::Multi { actions } => actions.iter().any(uses_obs),
        Action::Toggle { a, b } => uses_obs(a) || uses_obs(b),
        _ => false,
    }
}

/// Starts the connection loop once (it idles while OBS is disabled in the settings).
pub fn ensure_started() {
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    let (tx, rx) = mpsc::unbounded_channel();
    *REQUESTS.lock() = Some(tx);
    tauri::async_runtime::spawn(connection_loop(rx));
}

async fn connection_loop(mut rx: mpsc::UnboundedReceiver<Req>) {
    let mut backoff = Duration::from_secs(1);
    loop {
        let cfg = config();
        let Some(cfg) = cfg.filter(|c| c.enabled) else {
            // Disabled: answer queued requests with an error and wait.
            fail_pending(&mut rx, "OBS is turned off in the settings");
            tokio::time::sleep(Duration::from_secs(2)).await;
            continue;
        };
        match run_session(&cfg, &mut rx).await {
            Ok(()) => backoff = Duration::from_secs(1),
            Err(e) => {
                tracing::debug!("OBS connection: {e:#}");
                set_status(|s| s.connected = false);
                fail_pending(&mut rx, "OBS is not reachable");
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(Duration::from_secs(30));
            }
        }
    }
}

fn fail_pending(rx: &mut mpsc::UnboundedReceiver<Req>, msg: &str) {
    while let Ok(r) = rx.try_recv() {
        let _ = r.reply.send(Err(anyhow!("{msg}")));
    }
}

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Connects and completes the Hello / Identify handshake. Returns the socket.
pub async fn connect(cfg: &ObsConfig) -> anyhow::Result<Ws> {
    let url = format!("ws://{}:{}", cfg.host.trim(), cfg.port);
    let (mut ws, _) = tokio::time::timeout(
        Duration::from_secs(3),
        tokio_tungstenite::connect_async(&url),
    )
    .await
    .context("timed out connecting to OBS")?
    .with_context(|| format!("cannot connect to {url}"))?;

    let hello = next_json(&mut ws).await?;
    if hello["op"] != OpCode::Hello as u8 {
        bail!("unexpected first message from OBS");
    }
    let auth = hello["d"].get("authentication");
    let mut identify = json!({
        "rpcVersion": 1,
        "eventSubscriptions": protocol::EVENT_SUBSCRIPTIONS,
    });
    if let Some(auth) = auth {
        if cfg.password.is_empty() {
            bail!("OBS requires a password");
        }
        let challenge = auth["challenge"].as_str().unwrap_or_default();
        let salt = auth["salt"].as_str().unwrap_or_default();
        identify["authentication"] = json!(auth_string(&cfg.password, salt, challenge));
    }
    ws.send(Message::Text(
        json!({ "op": OpCode::Identify as u8, "d": identify })
            .to_string()
            .into(),
    ))
    .await?;
    let identified = next_json(&mut ws).await?;
    if identified["op"] != OpCode::Identified as u8 {
        bail!("OBS rejected the connection (wrong password?)");
    }
    Ok(ws)
}

async fn next_json(ws: &mut Ws) -> anyhow::Result<Value> {
    loop {
        let msg = tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .context("OBS did not answer")?
            .ok_or_else(|| anyhow!("OBS closed the connection"))??;
        match msg {
            Message::Text(t) => return Ok(serde_json::from_str(&t)?),
            Message::Close(f) => {
                let reason = f.map(|f| f.reason.to_string()).unwrap_or_default();
                bail!("OBS closed the connection {reason}");
            }
            _ => continue,
        }
    }
}

fn request_msg(id: &str, kind: &str, data: Value) -> Message {
    Message::Text(
        json!({
            "op": OpCode::Request as u8,
            "d": { "requestType": kind, "requestId": id, "requestData": data }
        })
        .to_string()
        .into(),
    )
}

/// One connected session: initial state, then requests and events until it drops.
async fn run_session(cfg: &ObsConfig, rx: &mut mpsc::UnboundedReceiver<Req>) -> anyhow::Result<()> {
    let ws = connect(cfg).await?;
    tracing::info!("connected to OBS at {}:{}", cfg.host, cfg.port);
    let (mut sink, mut stream) = ws.split();
    let mut pending: std::collections::HashMap<String, oneshot::Sender<anyhow::Result<Value>>> =
        Default::default();
    let mut next_id = 0u64;

    // Initial state.
    for (id, kind) in [
        ("init-rec", "GetRecordStatus"),
        ("init-stream", "GetStreamStatus"),
        ("init-inputs", "GetInputList"),
    ] {
        sink.send(request_msg(id, kind, json!({}))).await?;
    }
    set_status(|s| s.connected = true);
    let cfg_snapshot = cfg.clone();

    loop {
        tokio::select! {
            req = rx.recv() => {
                let Some(req) = req else { return Ok(()) };
                next_id += 1;
                let id = format!("r{next_id}");
                sink.send(request_msg(&id, &req.kind, req.data)).await?;
                pending.insert(id, req.reply);
            }
            msg = stream.next() => {
                let msg = msg.ok_or_else(|| anyhow!("OBS disconnected"))??;
                let Message::Text(text) = msg else {
                    if let Message::Close(_) = msg { bail!("OBS closed the connection"); }
                    continue;
                };
                let v: Value = serde_json::from_str(&text)?;
                let d = &v["d"];
                match v["op"].as_u64() {
                    Some(op) if op == OpCode::Event as u64 => handle_event(d),
                    Some(op) if op == OpCode::RequestResponse as u64 => {
                        let id = d["requestId"].as_str().unwrap_or_default().to_string();
                        let ok = d["requestStatus"]["result"].as_bool().unwrap_or(false);
                        let data = d.get("responseData").cloned().unwrap_or(Value::Null);
                        match id.as_str() {
                            "init-rec" => set_status(|s| s.recording = data["outputActive"].as_bool().unwrap_or(false)),
                            "init-stream" => set_status(|s| s.streaming = data["outputActive"].as_bool().unwrap_or(false)),
                            "init-inputs" => {
                                // Ask for the mute state of every input.
                                if let Some(inputs) = data["inputs"].as_array() {
                                    for (i, input) in inputs.iter().enumerate() {
                                        if let Some(name) = input["inputName"].as_str() {
                                            sink.send(request_msg(&format!("mute-{i}"), "GetInputMute", json!({ "inputName": name }))).await?;
                                            pending_mute_names().lock().insert(format!("mute-{i}"), name.to_string());
                                        }
                                    }
                                }
                            }
                            other if other.starts_with("mute-") => {
                                if let Some(name) = pending_mute_names().lock().remove(other) {
                                    let muted = data["inputMuted"].as_bool().unwrap_or(false);
                                    set_status(|s| { if muted { s.muted_sources.insert(name); } else { s.muted_sources.remove(&name); } });
                                }
                            }
                            _ => {
                                if let Some(reply) = pending.remove(&id) {
                                    let result = if ok {
                                        Ok(data)
                                    } else {
                                        let comment = d["requestStatus"]["comment"].as_str().unwrap_or("request failed");
                                        Err(anyhow!("OBS: {comment}"))
                                    };
                                    let _ = reply.send(result);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(2)) => {
                // Settings changed (disabled or other server): reconnect.
                if config().as_ref() != Some(&cfg_snapshot) {
                    let _ = sink.send(Message::Close(None)).await;
                    set_status(|s| s.connected = false);
                    return Ok(());
                }
            }
        }
    }
}

fn pending_mute_names() -> &'static Mutex<std::collections::HashMap<String, String>> {
    static M: std::sync::OnceLock<Mutex<std::collections::HashMap<String, String>>> =
        std::sync::OnceLock::new();
    M.get_or_init(Default::default)
}

fn handle_event(d: &Value) {
    let data = &d["eventData"];
    match d["eventType"].as_str() {
        Some("RecordStateChanged") => {
            set_status(|s| s.recording = data["outputActive"].as_bool().unwrap_or(false))
        }
        Some("StreamStateChanged") => {
            set_status(|s| s.streaming = data["outputActive"].as_bool().unwrap_or(false))
        }
        Some("InputMuteStateChanged") => {
            if let Some(name) = data["inputName"].as_str() {
                let muted = data["inputMuted"].as_bool().unwrap_or(false);
                set_status(|s| {
                    if muted {
                        s.muted_sources.insert(name.to_string());
                    } else {
                        s.muted_sources.remove(name);
                    }
                });
            }
        }
        _ => {}
    }
}

/// Sends a request through the shared connection (connecting if needed).
pub async fn request(kind: &str, data: Value) -> anyhow::Result<Value> {
    ensure_started();
    let tx = REQUESTS
        .lock()
        .clone()
        .ok_or_else(|| anyhow!("OBS client not running"))?;
    let (reply, rx) = oneshot::channel();
    tx.send(Req {
        kind: kind.to_string(),
        data,
        reply,
    })
    .map_err(|_| anyhow!("OBS client stopped"))?;
    tokio::time::timeout(Duration::from_secs(5), rx)
        .await
        .context("OBS did not answer in time")?
        .map_err(|_| anyhow!("OBS connection dropped"))?
}

/// Runs an OBS action (blocking; called from the action worker thread).
pub fn run(op: ObsOp, scene: &str, source: &str) -> anyhow::Result<()> {
    let (kind, data) = protocol::request_for(op, scene, source)?;
    tauri::async_runtime::block_on(request(kind, data))?;
    // Optimistic update so the button state is right even before the event arrives.
    match op {
        ObsOp::StartRecord => set_status(|s| s.recording = true),
        ObsOp::StopRecord => set_status(|s| s.recording = false),
        ObsOp::StartStream => set_status(|s| s.streaming = true),
        ObsOp::StopStream => set_status(|s| s.streaming = false),
        _ => {}
    }
    Ok(())
}

/// Connects once with the given settings and returns the OBS version (settings "Test").
pub async fn test(cfg: &ObsConfig) -> anyhow::Result<String> {
    let mut ws = connect(cfg).await?;
    ws.send(request_msg("v", "GetVersion", json!({}))).await?;
    loop {
        let v = next_json(&mut ws).await?;
        if v["op"] == OpCode::RequestResponse as u8 {
            let _ = ws.close(None).await;
            return Ok(v["d"]["responseData"]["obsVersion"]
                .as_str()
                .unwrap_or("?")
                .to_string());
        }
    }
}

/// Scene and input names for the editor.
pub async fn lists() -> anyhow::Result<(Vec<String>, Vec<String>)> {
    let scenes = request("GetSceneList", json!({})).await?;
    let inputs = request("GetInputList", json!({})).await?;
    let names = |v: &Value, arr: &str, key: &str| -> Vec<String> {
        v[arr]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| x[key].as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut scene_names = names(&scenes, "scenes", "sceneName");
    // OBS lists scenes bottom-up; show them as in the OBS UI.
    scene_names.reverse();
    Ok((scene_names, names(&inputs, "inputs", "inputName")))
}
