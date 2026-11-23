//! Loading, saving, migrating and selecting profiles.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context};
use serde_json::Value;

use super::model::{Action, Button, Page, Profile, DEFAULT_PROFILE_ID, PROFILE_VERSION};
use crate::fsutil;

/// Upgrades a profile JSON of any older version to the current one.
pub fn migrate(mut value: Value) -> anyhow::Result<Value> {
    let obj = value
        .as_object_mut()
        .ok_or_else(|| anyhow!("profile is not a JSON object"))?;
    let version = obj.get("version").and_then(Value::as_u64).unwrap_or(0) as u32;
    if version > PROFILE_VERSION {
        bail!("profile version {version} is newer than supported {PROFILE_VERSION}");
    }
    if version < 1 {
        // v0 (pre-release drafts) had no version field and no fillFromDefault.
        obj.entry("fillFromDefault").or_insert(Value::Bool(true));
        obj.insert("version".into(), Value::from(1));
    }
    // Future: `if version < 2 { ... }`
    Ok(value)
}

pub fn parse_profile(text: &str) -> anyhow::Result<Profile> {
    let value: Value = serde_json::from_str(text).context("invalid JSON")?;
    let value = migrate(value)?;
    let mut profile: Profile = serde_json::from_value(value).context("invalid profile")?;
    profile.page.normalize(profile.layout.slot_count());
    profile.apps = profile
        .apps
        .iter()
        .map(|a| normalize_exe(a))
        .filter(|a| !a.is_empty())
        .collect();
    Ok(profile)
}

/// Lower-case file name only (`C:\x\Chrome.EXE` -> `chrome.exe`).
pub fn normalize_exe(exe: &str) -> String {
    let name = exe.rsplit(['\\', '/']).next().unwrap_or(exe);
    name.trim().to_lowercase()
}

fn safe_file_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub struct ProfileStore {
    dir: PathBuf,
    profiles: Vec<Profile>,
}

impl ProfileStore {
    /// Loads all profiles from `dir`. Corrupt files are renamed to `.broken` (SPEC 10.3).
    /// `make_default` creates the default profile on first run.
    pub fn load(dir: &Path, make_default: impl FnOnce() -> Profile) -> Self {
        let _ = std::fs::create_dir_all(dir);
        let mut profiles = Vec::new();
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                let result = std::fs::read_to_string(&path)
                    .map_err(anyhow::Error::from)
                    .and_then(|t| parse_profile(&t));
                match result {
                    Ok(p) => profiles.push(p),
                    Err(e) => {
                        let broken = path.with_extension("json.broken");
                        tracing::error!(
                            "profile {} is broken ({e:#}); renamed to {}",
                            path.display(),
                            broken.display()
                        );
                        let _ = std::fs::rename(&path, &broken);
                    }
                }
            }
        }
        let mut store = Self {
            dir: dir.to_path_buf(),
            profiles,
        };
        if store.get(DEFAULT_PROFILE_ID).is_none() {
            let p = make_default();
            if let Err(e) = store.save(p) {
                tracing::error!("cannot save default profile: {e:#}");
            }
        }
        store.sort();
        store
    }

    /// In-memory store (tests).
    pub fn from_profiles(profiles: Vec<Profile>) -> Self {
        let mut s = Self {
            dir: PathBuf::new(),
            profiles,
        };
        s.sort();
        s
    }

    fn sort(&mut self) {
        self.profiles.sort_by(|a, b| {
            b.is_default()
                .cmp(&a.is_default())
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
    }

    pub fn list(&self) -> &[Profile] {
        &self.profiles
    }

    pub fn get(&self, id: &str) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    pub fn default_profile(&self) -> &Profile {
        self.get(DEFAULT_PROFILE_ID)
            .or_else(|| self.profiles.first())
            .expect("default profile always exists")
    }

    fn path_for(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    /// Inserts or replaces a profile and writes it to disk atomically.
    pub fn save(&mut self, mut profile: Profile) -> anyhow::Result<()> {
        if !safe_file_id(&profile.id) {
            bail!("invalid profile id {:?}", profile.id);
        }
        profile.version = PROFILE_VERSION;
        profile.page.normalize(profile.layout.slot_count());
        profile.apps = profile.apps.iter().map(|a| normalize_exe(a)).collect();
        if profile.is_default() {
            profile.apps.clear();
        }
        if !self.dir.as_os_str().is_empty() {
            fsutil::write_json_atomic(&self.path_for(&profile.id), &profile)?;
        }
        match self.profiles.iter_mut().find(|p| p.id == profile.id) {
            Some(slot) => *slot = profile,
            None => self.profiles.push(profile),
        }
        self.sort();
        Ok(())
    }

    pub fn delete(&mut self, id: &str) -> anyhow::Result<()> {
        if id == DEFAULT_PROFILE_ID {
            bail!("the default profile cannot be deleted");
        }
        if !safe_file_id(id) {
            bail!("invalid profile id");
        }
        self.profiles.retain(|p| p.id != id);
        if !self.dir.as_os_str().is_empty() {
            let path = self.path_for(id);
            if path.exists() {
                std::fs::remove_file(path)?;
            }
        }
        Ok(())
    }

    /// The profile for an executable name, or the default profile (SPEC 5).
    pub fn select_for_exe(&self, exe: Option<&str>) -> &Profile {
        if let Some(exe) = exe {
            let exe = normalize_exe(exe);
            if let Some(p) = self
                .profiles
                .iter()
                .find(|p| !p.is_default() && p.apps.contains(&exe))
            {
                return p;
            }
        }
        self.default_profile()
    }

    /// The page shown in the panel: the profile's page at `path`, with empty top-level
    /// slots filled from the default profile when the profile asks for it.
    pub fn effective_page(&self, profile: &Profile, path: &[usize]) -> Option<Page> {
        let page = profile.page_at(path)?.clone();
        if !path.is_empty() || profile.is_default() || !profile.fill_from_default {
            return Some(page);
        }
        Some(fill_from(&page, &self.default_profile().page))
    }
}

/// Fills empty slots of `page` with the button at the same index in `fallback`.
pub fn fill_from(page: &Page, fallback: &Page) -> Page {
    let slots = page
        .slots
        .iter()
        .enumerate()
        .map(|(i, s)| match s {
            Some(b) => Some(b.clone()),
            None => fallback.slots.get(i).cloned().flatten(),
        })
        .collect();
    Page { slots }
}

/// Finds a button by id anywhere in the profile.
pub fn find_button<'a>(profile: &'a Profile, id: &str) -> Option<&'a Button> {
    let mut found = None;
    profile.for_each_button(&mut |b| {
        if found.is_none() && b.id == id {
            found = Some(b);
        }
    });
    found
}

/// Replaces all button ids (used when importing or duplicating so ids stay unique).
pub fn regenerate_ids(page: &mut Page) {
    for b in page.slots.iter_mut().flatten() {
        b.id = super::model::new_id();
        if let Action::Folder { page } = &mut b.action {
            regenerate_ids(page);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::KeyCombo;
    use crate::profiles::model::{Layout, MediaOp};

    fn button(title: &str) -> Button {
        Button::new(title, "star", Action::Media { op: MediaOp::Next })
    }

    fn store() -> ProfileStore {
        let mut def = Profile::new(DEFAULT_PROFILE_ID, "Default", Layout::Radial);
        def.page.slots[0] = Some(button("d0"));
        def.page.slots[1] = Some(button("d1"));
        let mut browser = Profile::new("browser", "Browser", Layout::Radial);
        browser.apps = vec!["chrome.exe".into(), "msedge.exe".into()];
        browser.page.slots[1] = Some(button("b1"));
        let mut code = Profile::new("code", "Code", Layout::Radial);
        code.apps = vec!["code.exe".into()];
        code.fill_from_default = false;
        ProfileStore::from_profiles(vec![browser, def, code])
    }

    #[test]
    fn selects_profile_by_exe_case_insensitive() {
        let s = store();
        assert_eq!(s.select_for_exe(Some("CHROME.EXE")).id, "browser");
        assert_eq!(s.select_for_exe(Some("msedge.exe")).id, "browser");
        assert_eq!(
            s.select_for_exe(Some(r"C:\Program Files\Code\Code.exe")).id,
            "code"
        );
        assert_eq!(
            s.select_for_exe(Some("explorer.exe")).id,
            DEFAULT_PROFILE_ID
        );
        assert_eq!(s.select_for_exe(None).id, DEFAULT_PROFILE_ID);
    }

    #[test]
    fn default_profile_sorted_first() {
        let s = store();
        assert_eq!(s.list()[0].id, DEFAULT_PROFILE_ID);
    }

    #[test]
    fn fills_empty_slots_from_default() {
        let s = store();
        let browser = s.get("browser").unwrap();
        let page = s.effective_page(browser, &[]).unwrap();
        assert_eq!(page.slots[0].as_ref().unwrap().title, "d0");
        assert_eq!(page.slots[1].as_ref().unwrap().title, "b1");
        assert!(page.slots[2].is_none());

        let code = s.get("code").unwrap();
        let page = s.effective_page(code, &[]).unwrap();
        assert!(page.slots[0].is_none());
    }

    #[test]
    fn folder_path_navigation() {
        let mut p = Profile::new("x", "X", Layout::Radial);
        let mut inner = Page::with_slots(8);
        inner.slots[3] = Some(button("inner"));
        p.page.slots[2] = Some(Button::new(
            "Folder",
            "folder",
            Action::Folder { page: inner },
        ));
        assert_eq!(
            p.page_at(&[2]).unwrap().slots[3].as_ref().unwrap().title,
            "inner"
        );
        assert!(p.page_at(&[0]).is_none());
        assert!(p.page_at(&[2, 3]).is_none());
    }

    #[test]
    fn serialization_roundtrip() {
        let mut p = Profile::new("rt", "Round trip", Layout::Grid { cols: 3, rows: 2 });
        p.apps = vec!["notepad.exe".into()];
        p.page.slots[0] = Some(Button::new(
            "Select all",
            "text-select",
            Action::hotkey(KeyCombo::new(true, false, false, false, "A")),
        ));
        p.page.slots[1] = Some(Button::new(
            "Toggle",
            "toggle-left",
            Action::Toggle {
                a: Box::new(Action::Media { op: MediaOp::Next }),
                b: Box::new(Action::Delay { ms: 10 }),
            },
        ));
        let json = serde_json::to_string_pretty(&p).unwrap();
        assert!(json.contains("\"type\": \"hotkey\""));
        assert!(json.contains("\"kind\": \"grid\""));
        assert!(json.contains("\"fillFromDefault\": true"));
        let back = parse_profile(&json).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn migrates_v0_profile() {
        let v0 = r#"{"id":"old","name":"Old","layout":{"kind":"radial"},
            "page":{"slots":[{"id":"b","title":"Next","action":{"type":"media","op":"next"}}]}}"#;
        let p = parse_profile(v0).unwrap();
        assert_eq!(p.version, PROFILE_VERSION);
        assert!(p.fill_from_default);
        assert_eq!(p.page.slots.len(), 8, "normalized to the layout slot count");
    }

    #[test]
    fn rejects_newer_version() {
        let json = r#"{"version": 999, "id":"x","name":"X"}"#;
        assert!(parse_profile(json).is_err());
    }

    #[test]
    fn broken_file_is_renamed_and_others_load() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("bad.json"), "{ not json").unwrap();
        let good = Profile::new("good", "Good", Layout::Radial);
        std::fs::write(
            dir.path().join("good.json"),
            serde_json::to_string(&good).unwrap(),
        )
        .unwrap();
        let s = ProfileStore::load(dir.path(), || {
            Profile::new(DEFAULT_PROFILE_ID, "Default", Layout::Radial)
        });
        assert!(dir.path().join("bad.json.broken").exists());
        assert!(s.get("good").is_some());
        assert!(s.get(DEFAULT_PROFILE_ID).is_some());
        assert!(dir.path().join("default.json").exists());
    }

    #[test]
    fn default_profile_cannot_be_deleted() {
        let mut s = store();
        assert!(s.delete(DEFAULT_PROFILE_ID).is_err());
        assert!(s.delete("browser").is_ok());
        assert!(s.get("browser").is_none());
    }

    #[test]
    fn radial_coordinates_ring() {
        let l = Layout::Radial;
        assert_eq!(l.coordinates(0), (1, 0));
        assert_eq!(l.coordinates(2), (2, 1));
        assert_eq!(l.coordinates(6), (0, 1));
        let g = Layout::Grid { cols: 4, rows: 3 };
        assert_eq!(g.coordinates(5), (1, 1));
        assert_eq!(g.slot_count(), 12);
    }
}
