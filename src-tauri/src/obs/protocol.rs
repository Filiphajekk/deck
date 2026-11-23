//! Pure parts of the obs-websocket v5 protocol (unit-tested).

use anyhow::bail;
use base64::Engine;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::profiles::model::ObsOp;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OpCode {
    Hello = 0,
    Identify = 1,
    Identified = 2,
    Event = 5,
    Request = 6,
    RequestResponse = 7,
}

/// Scenes (1<<2) | Inputs (1<<3) | Outputs (1<<6).
pub const EVENT_SUBSCRIPTIONS: u32 = (1 << 2) | (1 << 3) | (1 << 6);

fn b64_sha256(s: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(Sha256::digest(s.as_bytes()))
}

/// Authentication string: base64(sha256(base64(sha256(password + salt)) + challenge)).
pub fn auth_string(password: &str, salt: &str, challenge: &str) -> String {
    let secret = b64_sha256(&format!("{password}{salt}"));
    b64_sha256(&format!("{secret}{challenge}"))
}

/// Request type and data for an action.
pub fn request_for(op: ObsOp, scene: &str, source: &str) -> anyhow::Result<(&'static str, Value)> {
    Ok(match op {
        ObsOp::SwitchScene => {
            if scene.trim().is_empty() {
                bail!("no scene selected");
            }
            ("SetCurrentProgramScene", json!({ "sceneName": scene }))
        }
        ObsOp::ToggleRecord => ("ToggleRecord", json!({})),
        ObsOp::StartRecord => ("StartRecord", json!({})),
        ObsOp::StopRecord => ("StopRecord", json!({})),
        ObsOp::ToggleStream => ("ToggleStream", json!({})),
        ObsOp::StartStream => ("StartStream", json!({})),
        ObsOp::StopStream => ("StopStream", json!({})),
        ObsOp::ToggleMute => {
            if source.trim().is_empty() {
                bail!("no source selected");
            }
            ("ToggleInputMute", json!({ "inputName": source }))
        }
        ObsOp::SaveReplay => ("SaveReplayBuffer", json!({})),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_matches_protocol_example() {
        // Independently computed with Node.js crypto for the same inputs.
        let expected = "AjGKOwfTBa3TzjC6/bnd4VgPeOhmgaScVGc50Gy8k0c=";
        assert_eq!(
            auth_string(
                "supersecretpassword",
                "lZFm6Qr+2GlRMqC/ekXP4cO7fW0YMd/zVhC4MBd7gKA=",
                "+IxH4CnCiqpX1rM9scsNynZzbOe4KhDeYcTNS3PDaeY="
            ),
            expected
        );
        // Base64 of a SHA-256 digest is always 44 characters.
        assert_eq!(expected.len(), 44);
    }

    #[test]
    fn request_mapping() {
        assert_eq!(
            request_for(ObsOp::ToggleRecord, "", "").unwrap().0,
            "ToggleRecord"
        );
        let (k, d) = request_for(ObsOp::SwitchScene, "Game", "").unwrap();
        assert_eq!(k, "SetCurrentProgramScene");
        assert_eq!(d["sceneName"], "Game");
        assert!(request_for(ObsOp::SwitchScene, " ", "").is_err());
        assert!(request_for(ObsOp::ToggleMute, "", "").is_err());
    }
}
