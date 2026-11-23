//! Global settings (`config.json`). Mirrored in `src/types/config.ts`.

use std::path::Path;

use anyhow::{anyhow, bail, Context};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::fsutil;
use crate::input::hook::MouseTrigger;
use crate::keys::KeyCombo;
use crate::lang::LanguagePref;

pub const CONFIG_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Appearance {
    /// Panel scale in percent (60–200).
    pub size: u16,
    /// Background opacity 0.3–1.0.
    pub opacity: f32,
    /// Highlight colour as `#rrggbb`.
    pub accent: String,
    pub animations: bool,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            size: 100,
            opacity: 0.88,
            accent: "#38bdf8".into(),
            animations: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ObsConfig {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub password: String,
}

impl Default for ObsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            host: "127.0.0.1".into(),
            port: 4455,
            password: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub version: u32,
    pub hotkey: KeyCombo,
    pub mouse_trigger: MouseTrigger,
    pub hold_threshold_ms: u32,
    pub appearance: Appearance,
    pub language: LanguagePref,
    pub autostart: bool,
    pub run_as_admin: bool,
    pub obs: ObsConfig,
    pub first_run_done: bool,
    /// The "run as administrator" hint for elevated windows was already shown once.
    pub uipi_hint_shown: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            hotkey: KeyCombo::new(true, true, false, false, "Space"),
            mouse_trigger: MouseTrigger::None,
            hold_threshold_ms: 200,
            appearance: Appearance::default(),
            language: LanguagePref::Auto,
            autostart: true,
            run_as_admin: false,
            obs: ObsConfig::default(),
            first_run_done: false,
            uipi_hint_shown: false,
        }
    }
}

impl Config {
    /// Clamps values into sane ranges; rejects a hotkey without a key.
    pub fn sanitize(&mut self) {
        self.hold_threshold_ms = self.hold_threshold_ms.clamp(50, 2000);
        self.appearance.size = self.appearance.size.clamp(60, 200);
        self.appearance.opacity = self.appearance.opacity.clamp(0.3, 1.0);
        let accent_ok = self.appearance.accent.len() == 7
            && self.appearance.accent.starts_with('#')
            && self.appearance.accent[1..]
                .chars()
                .all(|c| c.is_ascii_hexdigit());
        if !accent_ok {
            self.appearance.accent = Appearance::default().accent;
        }
        if self.hotkey.vk().is_none() {
            self.hotkey = Config::default().hotkey;
        }
    }
}

pub fn migrate(mut value: Value) -> anyhow::Result<Value> {
    let obj = value
        .as_object_mut()
        .ok_or_else(|| anyhow!("config is not an object"))?;
    let version = obj.get("version").and_then(Value::as_u64).unwrap_or(0) as u32;
    if version > CONFIG_VERSION {
        bail!("config version {version} is newer than supported");
    }
    obj.insert("version".into(), Value::from(CONFIG_VERSION));
    Ok(value)
}

/// Loads the config; a missing file gives defaults, a corrupt one is renamed to `.broken`.
pub fn load(path: &Path) -> Config {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return Config::default(),
    };
    let parsed = serde_json::from_str::<Value>(&text)
        .context("invalid JSON")
        .and_then(migrate)
        .and_then(|v| serde_json::from_value::<Config>(v).context("invalid config"));
    match parsed {
        Ok(mut c) => {
            c.sanitize();
            c
        }
        Err(e) => {
            tracing::error!("config.json is broken ({e:#}); using defaults");
            let _ = std::fs::rename(path, path.with_extension("json.broken"));
            Config::default()
        }
    }
}

pub fn save(path: &Path, config: &Config) -> anyhow::Result<()> {
    fsutil::write_json_atomic(path, config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_get_defaults() {
        let v = migrate(serde_json::json!({ "holdThresholdMs": 300 })).unwrap();
        let c: Config = serde_json::from_value(v).unwrap();
        assert_eq!(c.hold_threshold_ms, 300);
        assert_eq!(c.hotkey.key.as_deref(), Some("Space"));
        assert!(c.autostart);
        assert_eq!(c.version, CONFIG_VERSION);
    }

    #[test]
    fn sanitize_clamps() {
        let mut c = Config {
            hold_threshold_ms: 5,
            ..Default::default()
        };
        c.appearance.accent = "red".into();
        c.hotkey.key = None;
        c.sanitize();
        assert_eq!(c.hold_threshold_ms, 50);
        assert_eq!(c.appearance.accent, "#38bdf8");
        assert!(c.hotkey.key.is_some());
    }

    #[test]
    fn roundtrip_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.json");
        let c = Config {
            mouse_trigger: MouseTrigger::X2,
            ..Default::default()
        };
        save(&p, &c).unwrap();
        assert_eq!(load(&p), c);
    }
}
