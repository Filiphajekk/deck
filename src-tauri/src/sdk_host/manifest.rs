//! Stream Deck plugin `manifest.json` (SDK v1/v2 fields we use).

use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
pub struct ManifestState {
    pub image: String,
    pub title: String,
    pub multi_action_image: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
pub struct ManifestAction {
    #[serde(rename = "UUID")]
    pub uuid: String,
    pub name: String,
    pub icon: String,
    pub tooltip: String,
    pub states: Vec<ManifestState>,
    pub property_inspector_path: Option<String>,
    pub controllers: Option<Vec<String>>,
    pub disable_automatic_states: bool,
    pub supported_in_multi_actions: Option<bool>,
    pub visible_in_action_list: Option<bool>,
}

impl ManifestAction {
    /// Keypad actions (or actions without a controller list) are supported; Encoder-only
    /// (Stream Deck + dials) actions are not (SPEC 7.1).
    pub fn is_keypad(&self) -> bool {
        match &self.controllers {
            None => true,
            Some(c) if c.is_empty() => true,
            Some(c) => c.iter().any(|x| x.eq_ignore_ascii_case("keypad")),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
pub struct NodeInfo {
    pub version: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct AppsToMonitor {
    pub windows: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
pub struct OsEntry {
    pub platform: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
pub struct Manifest {
    #[serde(rename = "UUID")]
    pub uuid: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub icon: String,
    pub category: String,
    pub category_icon: String,
    pub code_path: Option<String>,
    pub code_path_win: Option<String>,
    pub property_inspector_path: Option<String>,
    pub actions: Vec<ManifestAction>,
    pub nodejs: Option<NodeInfo>,
    pub applications_to_monitor: Option<AppsToMonitor>,
    #[serde(rename = "SDKVersion")]
    pub sdk_version: Option<u32>,
    #[serde(rename = "OS")]
    pub os: Vec<OsEntry>,
}

/// How the plugin's code is started.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Runtime {
    Node { script: String },
    Exe { exe: String },
    Html { page: String },
}

impl Manifest {
    pub fn load(dir: &Path) -> anyhow::Result<Manifest> {
        let path = dir.join("manifest.json");
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("{} has no manifest.json", dir.display()))?;
        // Some manifests start with a BOM.
        let text = text.trim_start_matches('\u{feff}');
        let mut m: Manifest = serde_json::from_str(text).context("invalid manifest.json")?;
        if m.uuid.is_empty() {
            // Old SDK v1 manifests have no UUID; the folder name is `<uuid>.sdPlugin`.
            m.uuid = uuid_from_dir(dir).unwrap_or_default();
        }
        if m.uuid.is_empty() {
            bail!("the plugin has no UUID");
        }
        Ok(m)
    }

    /// Code path to run on Windows, if the plugin supports Windows at all.
    pub fn windows_code_path(&self) -> Option<&str> {
        if !self.os.is_empty()
            && !self
                .os
                .iter()
                .any(|o| o.platform.eq_ignore_ascii_case("windows"))
        {
            return None;
        }
        self.code_path_win
            .as_deref()
            .or(self.code_path.as_deref())
            .filter(|p| !p.trim().is_empty())
    }

    pub fn runtime(&self) -> Option<Runtime> {
        let code = self.windows_code_path()?.replace('/', "\\");
        let lower = code.to_lowercase();
        if lower.ends_with(".html") || lower.ends_with(".htm") {
            Some(Runtime::Html { page: code })
        } else if self.nodejs.is_some()
            || lower.ends_with(".js")
            || lower.ends_with(".mjs")
            || lower.ends_with(".cjs")
        {
            Some(Runtime::Node { script: code })
        } else if lower.ends_with(".exe") || lower.ends_with(".cmd") || lower.ends_with(".bat") {
            Some(Runtime::Exe { exe: code })
        } else if !lower.contains('.') {
            // macOS-style binary name without extension: try `<name>.exe`.
            Some(Runtime::Exe {
                exe: format!("{code}.exe"),
            })
        } else {
            None
        }
    }

    pub fn action(&self, uuid: &str) -> Option<&ManifestAction> {
        self.actions.iter().find(|a| a.uuid == uuid)
    }
}

pub fn uuid_from_dir(dir: &Path) -> Option<String> {
    let name = dir.file_name()?.to_str()?;
    name.strip_suffix(".sdPlugin").map(String::from)
}

/// Resolves an image path from the manifest (given without extension) to a real file:
/// `<p>@2x.png`, `<p>.png`, `<p>.svg`, `<p>`.
pub fn resolve_image(plugin_dir: &Path, rel: &str) -> Option<PathBuf> {
    if rel.is_empty() {
        return None;
    }
    let base = plugin_dir.join(rel.replace('/', "\\"));
    let s = base.to_string_lossy().to_string();
    [
        format!("{s}@2x.png"),
        format!("{s}.png"),
        format!("{s}.svg"),
        s.clone(),
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    const V2: &str = r#"{
        "UUID": "com.example.counter",
        "Name": "Counter",
        "Version": "1.2.0.0",
        "SDKVersion": 2,
        "CodePath": "bin/plugin.js",
        "Nodejs": { "Version": "20" },
        "OS": [{ "Platform": "windows", "MinimumVersion": "10" }, { "Platform": "mac", "MinimumVersion": "12" }],
        "ApplicationsToMonitor": { "windows": ["Discord.exe"] },
        "Actions": [
            { "UUID": "com.example.counter.increment", "Name": "Count",
              "States": [{ "Image": "imgs/key", "Title": "0" }],
              "Controllers": ["Keypad"] },
            { "UUID": "com.example.counter.dial", "Name": "Dial",
              "States": [{ "Image": "imgs/dial" }], "Controllers": ["Encoder"] }
        ]
    }"#;

    fn parse(s: &str) -> Manifest {
        serde_json::from_str(s).unwrap()
    }

    #[test]
    fn parses_v2_manifest() {
        let m = parse(V2);
        assert_eq!(m.uuid, "com.example.counter");
        assert_eq!(m.sdk_version, Some(2));
        assert_eq!(
            m.runtime(),
            Some(Runtime::Node {
                script: r"bin\plugin.js".into()
            })
        );
        assert_eq!(
            m.applications_to_monitor.unwrap().windows,
            vec!["Discord.exe"]
        );
        assert!(m.actions[0].is_keypad());
        assert!(!m.actions[1].is_keypad());
    }

    #[test]
    fn windows_code_path_and_runtimes() {
        let mac_only = parse(r#"{"UUID":"a","CodePath":"plugin","OS":[{"Platform":"mac"}]}"#);
        assert_eq!(mac_only.runtime(), None);
        let exe = parse(r#"{"UUID":"a","CodePath":"mac/plugin","CodePathWin":"win/plugin.exe"}"#);
        assert_eq!(
            exe.runtime(),
            Some(Runtime::Exe {
                exe: r"win\plugin.exe".into()
            })
        );
        let html = parse(r#"{"UUID":"a","CodePath":"index.html"}"#);
        assert_eq!(
            html.runtime(),
            Some(Runtime::Html {
                page: "index.html".into()
            })
        );
        let none = parse(r#"{"UUID":"a"}"#);
        assert_eq!(none.runtime(), None);
    }

    #[test]
    fn uuid_from_folder_name() {
        assert_eq!(
            uuid_from_dir(Path::new(r"C:\x\com.foo.bar.sdPlugin")).as_deref(),
            Some("com.foo.bar")
        );
    }
}
