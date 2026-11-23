//! Integration test of the obs-websocket v5 client against a small mock OBS server
//! (real OBS is not available on the build machine).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use deckless_lib::config::ObsConfig;
use deckless_lib::profiles::model::ObsOp;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

const PASSWORD: &str = "secret-pass";
const SALT: &str = "c2FsdHNhbHQ=";
const CHALLENGE: &str = "Y2hhbGxlbmdl";

fn b64_sha(s: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(Sha256::digest(s.as_bytes()))
}

#[derive(Default)]
struct MockState {
    scene: Option<String>,
    recording: bool,
    identified: bool,
}

async fn mock_server(state: Arc<Mutex<MockState>>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let (tcp, _) = listener.accept().await.unwrap();
            let state = state.clone();
            tokio::spawn(async move {
                let mut ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
                ws.send(Message::Text(
                    json!({"op": 0, "d": {"obsWebSocketVersion": "5.5.0", "rpcVersion": 1,
                        "authentication": {"challenge": CHALLENGE, "salt": SALT}}})
                    .to_string()
                    .into(),
                ))
                .await
                .unwrap();
                while let Some(Ok(msg)) = ws.next().await {
                    let Message::Text(text) = msg else { continue };
                    let v: Value = serde_json::from_str(&text).unwrap();
                    match v["op"].as_u64().unwrap() {
                        1 => {
                            let expected = b64_sha(&format!(
                                "{}{}",
                                b64_sha(&format!("{PASSWORD}{SALT}")),
                                CHALLENGE
                            ));
                            if v["d"]["authentication"] != expected {
                                let _ = ws.close(None).await;
                                return;
                            }
                            state.lock().unwrap().identified = true;
                            ws.send(Message::Text(
                                json!({"op": 2, "d": {"negotiatedRpcVersion": 1}})
                                    .to_string()
                                    .into(),
                            ))
                            .await
                            .unwrap();
                        }
                        6 => {
                            let d = &v["d"];
                            let id = d["requestId"].clone();
                            let kind = d["requestType"].as_str().unwrap().to_string();
                            let mut event = None;
                            let data = match kind.as_str() {
                                "GetVersion" => json!({"obsVersion": "30.2.0"}),
                                "GetRecordStatus" => {
                                    json!({"outputActive": state.lock().unwrap().recording})
                                }
                                "GetStreamStatus" => json!({"outputActive": false}),
                                "GetInputList" => json!({"inputs": [{"inputName": "Mic"}]}),
                                "GetInputMute" => json!({"inputMuted": false}),
                                "GetSceneList" => {
                                    json!({"scenes": [{"sceneName": "Chat"}, {"sceneName": "Game"}]})
                                }
                                "SetCurrentProgramScene" => {
                                    state.lock().unwrap().scene =
                                        d["requestData"]["sceneName"].as_str().map(String::from);
                                    json!({})
                                }
                                "ToggleRecord" => {
                                    let mut s = state.lock().unwrap();
                                    s.recording = !s.recording;
                                    event = Some(
                                        json!({"op": 5, "d": {"eventType": "RecordStateChanged", "eventIntent": 64,
                                        "eventData": {"outputActive": s.recording, "outputState": "x"}}}),
                                    );
                                    json!({"outputActive": s.recording})
                                }
                                _ => json!({}),
                            };
                            ws.send(Message::Text(
                                json!({"op": 7, "d": {"requestType": kind, "requestId": id,
                                    "requestStatus": {"result": true, "code": 100}, "responseData": data}})
                                .to_string()
                                .into(),
                            ))
                            .await
                            .unwrap();
                            if let Some(e) = event {
                                ws.send(Message::Text(e.to_string().into())).await.unwrap();
                            }
                        }
                        _ => {}
                    }
                }
            });
        }
    });
    port
}

fn cfg(port: u16, password: &str) -> ObsConfig {
    ObsConfig {
        enabled: true,
        host: "127.0.0.1".into(),
        port,
        password: password.into(),
    }
}

#[test]
fn obs_client_against_mock_server() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let port = tauri::async_runtime::block_on(mock_server(state.clone()));

    // Settings "Test" button: authentication + GetVersion.
    let version =
        tauri::async_runtime::block_on(deckless_lib::obs::test(&cfg(port, PASSWORD))).unwrap();
    assert_eq!(version, "30.2.0");
    // Wrong password is reported.
    assert!(tauri::async_runtime::block_on(deckless_lib::obs::test(&cfg(port, "nope"))).is_err());

    // Shared connection used by actions.
    deckless_lib::obs::set_config_override(Some(cfg(port, PASSWORD)));
    deckless_lib::obs::run(ObsOp::SwitchScene, "Game", "").unwrap();
    assert_eq!(state.lock().unwrap().scene.as_deref(), Some("Game"));

    assert!(!deckless_lib::obs::status().recording);
    deckless_lib::obs::run(ObsOp::ToggleRecord, "", "").unwrap();
    // The RecordStateChanged event updates the live state shown on the button.
    let start = std::time::Instant::now();
    while !deckless_lib::obs::status().recording && start.elapsed() < Duration::from_secs(2) {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(deckless_lib::obs::status().recording);
    assert!(deckless_lib::obs::status().connected);

    let (scenes, inputs) = tauri::async_runtime::block_on(deckless_lib::obs::lists()).unwrap();
    assert_eq!(scenes, vec!["Game", "Chat"]);
    assert_eq!(inputs, vec!["Mic"]);
}
