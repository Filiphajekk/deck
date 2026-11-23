//! Persistent plugin settings: `plugin-settings\<uuid>.json` with global settings and
//! per-button settings (keyed by the button's context id).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSettings {
    pub version: u32,
    #[serde(default = "empty_object")]
    pub global: Value,
    #[serde(default)]
    pub buttons: BTreeMap<String, Value>,
}

fn empty_object() -> Value {
    Value::Object(Default::default())
}

impl Default for PluginSettings {
    fn default() -> Self {
        Self {
            version: 1,
            global: empty_object(),
            buttons: BTreeMap::new(),
        }
    }
}

pub struct SettingsStore {
    dir: PathBuf,
}

impl SettingsStore {
    pub fn new(dir: &Path) -> Self {
        let _ = std::fs::create_dir_all(dir);
        Self {
            dir: dir.to_path_buf(),
        }
    }

    fn path(&self, plugin: &str) -> PathBuf {
        self.dir.join(format!("{plugin}.json"))
    }

    pub fn load(&self, plugin: &str) -> PluginSettings {
        std::fs::read_to_string(self.path(plugin))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    fn save(&self, plugin: &str, s: &PluginSettings) {
        if let Err(e) = crate::fsutil::write_json_atomic(&self.path(plugin), s) {
            tracing::error!(plugin, "cannot save plugin settings: {e:#}");
        }
    }

    pub fn button(&self, plugin: &str, context: &str) -> Value {
        self.load(plugin)
            .buttons
            .get(context)
            .cloned()
            .unwrap_or_else(empty_object)
    }

    pub fn set_button(&self, plugin: &str, context: &str, settings: Value) {
        let mut s = self.load(plugin);
        s.buttons.insert(context.to_string(), settings);
        self.save(plugin, &s);
    }

    pub fn global(&self, plugin: &str) -> Value {
        self.load(plugin).global
    }

    pub fn set_global(&self, plugin: &str, settings: Value) {
        let mut s = self.load(plugin);
        s.global = settings;
        self.save(plugin, &s);
    }

    pub fn remove_plugin(&self, plugin: &str) {
        let _ = std::fs::remove_file(self.path(plugin));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn button_and_global_settings_persist() {
        let dir = tempfile::tempdir().unwrap();
        let s = SettingsStore::new(dir.path());
        assert_eq!(s.button("p", "c"), json!({}));
        s.set_button("p", "c", json!({"count": 2}));
        s.set_global("p", json!({"token": "x"}));
        let again = SettingsStore::new(dir.path());
        assert_eq!(again.button("p", "c")["count"], 2);
        assert_eq!(again.global("p")["token"], "x");
    }
}
