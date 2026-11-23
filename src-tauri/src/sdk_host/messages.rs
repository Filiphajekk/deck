//! Stream Deck SDK WebSocket messages: parsing what plugins / Property Inspectors send and
//! building what the host sends (SPEC 7.3, 7.4).

use serde_json::{json, Value};

/// Messages sent by a plugin or a Property Inspector.
#[derive(Debug, Clone, PartialEq)]
pub enum Incoming {
    RegisterPlugin {
        uuid: String,
    },
    RegisterPropertyInspector {
        uuid: String,
    },
    SetTitle {
        context: String,
        title: Option<String>,
        state: Option<u8>,
    },
    SetImage {
        context: String,
        image: Option<String>,
        state: Option<u8>,
    },
    SetState {
        context: String,
        state: u8,
    },
    ShowAlert {
        context: String,
    },
    ShowOk {
        context: String,
    },
    SetSettings {
        context: String,
        settings: Value,
    },
    GetSettings {
        context: String,
    },
    SetGlobalSettings {
        context: String,
        settings: Value,
    },
    GetGlobalSettings {
        context: String,
    },
    OpenUrl {
        url: String,
    },
    LogMessage {
        message: String,
    },
    SwitchToProfile {
        profile: String,
    },
    SendToPropertyInspector {
        action: String,
        context: String,
        payload: Value,
    },
    SendToPlugin {
        action: String,
        context: String,
        payload: Value,
    },
    Unknown {
        event: String,
    },
}

fn s(v: &Value, k: &str) -> String {
    v.get(k)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn opt_state(payload: &Value) -> Option<u8> {
    payload
        .get("state")
        .and_then(Value::as_u64)
        .map(|n| n.min(1) as u8)
}

pub fn parse(text: &str) -> anyhow::Result<Incoming> {
    let v: Value = serde_json::from_str(text)?;
    let event = s(&v, "event");
    let context = s(&v, "context");
    let payload = v.get("payload").cloned().unwrap_or(Value::Null);
    Ok(match event.as_str() {
        "registerPlugin" => Incoming::RegisterPlugin {
            uuid: s(&v, "uuid"),
        },
        "registerPropertyInspector" => Incoming::RegisterPropertyInspector {
            uuid: s(&v, "uuid"),
        },
        "setTitle" => Incoming::SetTitle {
            context,
            title: payload
                .get("title")
                .and_then(Value::as_str)
                .map(String::from),
            state: opt_state(&payload),
        },
        "setImage" => Incoming::SetImage {
            context,
            image: payload
                .get("image")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(String::from),
            state: opt_state(&payload),
        },
        "setState" => Incoming::SetState {
            context,
            state: opt_state(&payload).unwrap_or(0),
        },
        "showAlert" => Incoming::ShowAlert { context },
        "showOk" => Incoming::ShowOk { context },
        "setSettings" => Incoming::SetSettings {
            context,
            settings: payload,
        },
        "getSettings" => Incoming::GetSettings { context },
        "setGlobalSettings" => Incoming::SetGlobalSettings {
            context,
            settings: payload,
        },
        "getGlobalSettings" => Incoming::GetGlobalSettings { context },
        "openUrl" => Incoming::OpenUrl {
            url: s(&payload, "url"),
        },
        "logMessage" => Incoming::LogMessage {
            message: s(&payload, "message"),
        },
        "switchToProfile" => Incoming::SwitchToProfile {
            profile: s(&payload, "profile"),
        },
        "sendToPropertyInspector" => Incoming::SendToPropertyInspector {
            action: s(&v, "action"),
            context,
            payload,
        },
        "sendToPlugin" => Incoming::SendToPlugin {
            action: s(&v, "action"),
            context,
            payload,
        },
        _ => Incoming::Unknown { event },
    })
}

/// Everything about one button instance that events need.
#[derive(Debug, Clone, PartialEq)]
pub struct Instance {
    pub action: String,
    pub context: String,
    pub column: u8,
    pub row: u8,
    pub state: u8,
    pub settings: Value,
}

pub const DEVICE_ID: &str = "DECKLESS-VIRTUAL-0";

fn key_payload(i: &Instance) -> Value {
    json!({
        "settings": i.settings,
        "coordinates": { "column": i.column, "row": i.row },
        "state": i.state,
        "isInMultiAction": false,
        "controller": "Keypad",
    })
}

/// willAppear, willDisappear, keyDown, keyUp, didReceiveSettings.
pub fn instance_event(event: &str, i: &Instance) -> Value {
    json!({
        "action": i.action,
        "event": event,
        "context": i.context,
        "device": DEVICE_ID,
        "payload": key_payload(i),
    })
}

pub fn global_settings(settings: &Value) -> Value {
    json!({ "event": "didReceiveGlobalSettings", "payload": { "settings": settings } })
}

pub fn device_did_connect(columns: u8, rows: u8) -> Value {
    json!({
        "event": "deviceDidConnect",
        "device": DEVICE_ID,
        "deviceInfo": { "name": crate::branding::APP_NAME, "type": 0, "size": { "columns": columns, "rows": rows } },
    })
}

pub fn application_event(launched: bool, app: &str) -> Value {
    json!({
        "event": if launched { "applicationDidLaunch" } else { "applicationDidTerminate" },
        "payload": { "application": app },
    })
}

pub fn system_did_wake_up() -> Value {
    json!({ "event": "systemDidWakeUp" })
}

pub fn pi_event(event: &str, action: &str, context: &str) -> Value {
    json!({ "action": action, "event": event, "context": context, "device": DEVICE_ID })
}

pub fn send_to_plugin(action: &str, context: &str, payload: &Value) -> Value {
    json!({ "action": action, "event": "sendToPlugin", "context": context, "payload": payload })
}

pub fn send_to_pi(action: &str, context: &str, payload: &Value) -> Value {
    json!({ "action": action, "event": "sendToPropertyInspector", "context": context, "payload": payload })
}

pub fn title_parameters_did_change(i: &Instance, title: &str) -> Value {
    json!({
        "action": i.action,
        "event": "titleParametersDidChange",
        "context": i.context,
        "device": DEVICE_ID,
        "payload": {
            "coordinates": { "column": i.column, "row": i.row },
            "settings": i.settings,
            "state": i.state,
            "title": title,
            "titleParameters": {
                "fontFamily": "", "fontSize": 12, "fontStyle": "", "fontUnderline": false,
                "showTitle": true, "titleAlignment": "bottom", "titleColor": "#ffffff"
            }
        }
    })
}

/// The `-info` JSON given to plugins on start (SPEC 7.2).
pub fn info_json(
    plugin_uuid: &str,
    plugin_version: &str,
    language: &str,
    columns: u8,
    rows: u8,
) -> Value {
    json!({
        "application": {
            "font": "Segoe UI",
            "language": language,
            "platform": "windows",
            "platformVersion": "10.0",
            "version": HOST_VERSION,
        },
        "plugin": { "uuid": plugin_uuid, "version": plugin_version },
        "devicePixelRatio": 2,
        "colors": {
            "buttonPressedBackgroundColor": "#303030FF",
            "buttonPressedBorderColor": "#646464FF",
            "buttonPressedTextColor": "#969696FF",
            "disabledColor": "#F7821B59",
            "highlightColor": "#38BDF8FF",
            "mouseDownColor": "#0EA5E9FF",
        },
        "devices": [{
            "id": DEVICE_ID,
            "name": crate::branding::APP_NAME,
            "size": { "columns": columns, "rows": rows },
            "type": 0,
        }],
    })
}

/// Stream Deck application version reported to plugins. Plugins compare it with the
/// minimum version in their manifest, so it must be recent enough.
pub const HOST_VERSION: &str = "6.8.0";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plugin_messages() {
        assert_eq!(
            parse(r#"{"event":"registerPlugin","uuid":"abc"}"#).unwrap(),
            Incoming::RegisterPlugin { uuid: "abc".into() }
        );
        assert_eq!(
            parse(r#"{"event":"setTitle","context":"c1","payload":{"title":"5","target":0}}"#)
                .unwrap(),
            Incoming::SetTitle {
                context: "c1".into(),
                title: Some("5".into()),
                state: None
            }
        );
        assert_eq!(
            parse(r#"{"event":"setImage","context":"c1","payload":{"image":"","state":1}}"#)
                .unwrap(),
            Incoming::SetImage {
                context: "c1".into(),
                image: None,
                state: Some(1)
            }
        );
        assert_eq!(
            parse(r#"{"event":"setState","context":"c","payload":{"state":1}}"#).unwrap(),
            Incoming::SetState {
                context: "c".into(),
                state: 1
            }
        );
        assert_eq!(
            parse(r#"{"event":"setSettings","context":"c","payload":{"count":3}}"#).unwrap(),
            Incoming::SetSettings {
                context: "c".into(),
                settings: json!({"count": 3})
            }
        );
        assert_eq!(
            parse(r#"{"event":"openUrl","payload":{"url":"https://x.y"}}"#).unwrap(),
            Incoming::OpenUrl {
                url: "https://x.y".into()
            }
        );
        assert_eq!(
            parse(r#"{"event":"sendToPlugin","action":"a","context":"pi","payload":{"x":1}}"#)
                .unwrap(),
            Incoming::SendToPlugin {
                action: "a".into(),
                context: "pi".into(),
                payload: json!({"x":1})
            }
        );
        assert!(matches!(
            parse(r#"{"event":"whatever"}"#).unwrap(),
            Incoming::Unknown { .. }
        ));
        assert!(parse("not json").is_err());
    }

    #[test]
    fn builds_key_down() {
        let i = Instance {
            action: "com.a.b".into(),
            context: "ctx".into(),
            column: 2,
            row: 1,
            state: 0,
            settings: json!({"count": 1}),
        };
        let v = instance_event("keyDown", &i);
        assert_eq!(v["event"], "keyDown");
        assert_eq!(v["payload"]["coordinates"]["column"], 2);
        assert_eq!(v["payload"]["settings"]["count"], 1);
        assert_eq!(v["payload"]["isInMultiAction"], false);
        assert_eq!(v["device"], DEVICE_ID);
    }

    #[test]
    fn info_contains_device() {
        let v = info_json("u", "1.0", "cs", 3, 3);
        assert_eq!(v["application"]["platform"], "windows");
        assert_eq!(v["devices"][0]["size"]["columns"], 3);
        assert_eq!(v["plugin"]["uuid"], "u");
    }
}
