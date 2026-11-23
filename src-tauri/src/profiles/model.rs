//! Profile data model. Mirrored by hand in `src/types/profile.ts` — keep them in sync.

use serde::{Deserialize, Serialize};

use crate::keys::KeyCombo;

pub const PROFILE_VERSION: u32 = 1;
pub const DEFAULT_PROFILE_ID: &str = "default";
pub const RADIAL_SLOTS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[derive(Default)]
pub enum Layout {
    #[default]
    Radial,
    Grid {
        cols: u8,
        rows: u8,
    },
}

impl Layout {
    pub fn slot_count(self) -> usize {
        match self {
            Layout::Radial => RADIAL_SLOTS,
            Layout::Grid { cols, rows } => cols.clamp(2, 8) as usize * rows.clamp(2, 8) as usize,
        }
    }

    /// Size of the virtual Stream Deck reported to plugins (SPEC 7.2).
    pub fn device_size(self) -> (u8, u8) {
        match self {
            Layout::Radial => (3, 3),
            Layout::Grid { cols, rows } => (cols.clamp(2, 8), rows.clamp(2, 8)),
        }
    }

    /// Column/row of a slot on the virtual device. Radial slices go clockwise around the
    /// 3x3 ring starting at the top; the centre is not a slot.
    pub fn coordinates(self, slot: usize) -> (u8, u8) {
        match self {
            Layout::Radial => {
                const RING: [(u8, u8); 8] = [
                    (1, 0),
                    (2, 0),
                    (2, 1),
                    (2, 2),
                    (1, 2),
                    (0, 2),
                    (0, 1),
                    (0, 0),
                ];
                RING[slot % 8]
            }
            Layout::Grid { cols, .. } => {
                let cols = cols.clamp(2, 8) as usize;
                ((slot % cols) as u8, (slot / cols) as u8)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Icon {
    /// Built-in Lucide icon by kebab-case name.
    Lucide {
        name: String,
    },
    /// Data URL (PNG/JPG/SVG/GIF), already scaled to at most 144x144.
    Image {
        src: String,
    },
    Emoji {
        value: String,
    },
}

impl Icon {
    pub fn lucide(name: &str) -> Self {
        Icon::Lucide {
            name: name.to_string(),
        }
    }
}

/// Icon and title for one visual state of a button.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Visual {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub icon: Option<Icon>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Button {
    /// Stable id; also used as the plugin `context`.
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub icon: Option<Icon>,
    /// Look of the second state for toggles (muted mic, recording...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_b: Option<Visual>,
    pub action: Action,
}

impl Button {
    pub fn new(title: &str, icon: &str, action: Action) -> Self {
        Self {
            id: new_id(),
            title: title.to_string(),
            icon: Some(Icon::lucide(icon)),
            state_b: None,
            action,
        }
    }

    pub fn with_state_b(mut self, title: &str, icon: &str) -> Self {
        self.state_b = Some(Visual {
            title: title.to_string(),
            icon: Some(Icon::lucide(icon)),
        });
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    #[serde(default)]
    pub slots: Vec<Option<Button>>,
}

impl Page {
    pub fn with_slots(n: usize) -> Self {
        Self {
            slots: vec![None; n],
        }
    }

    /// Pads or truncates to `n` slots (after a layout change). Buttons beyond `n` are dropped.
    pub fn normalize(&mut self, n: usize) {
        self.slots.resize(n, None);
        for b in self.slots.iter_mut().flatten() {
            if let Action::Folder { page } = &mut b.action {
                page.normalize(n);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub version: u32,
    pub id: String,
    pub name: String,
    /// Executable names (lower case) this profile is used for. Empty for the default profile.
    #[serde(default)]
    pub apps: Vec<String>,
    #[serde(default)]
    pub layout: Layout,
    #[serde(default = "default_true")]
    pub fill_from_default: bool,
    #[serde(default)]
    pub page: Page,
}

fn default_true() -> bool {
    true
}

impl Profile {
    pub fn new(id: &str, name: &str, layout: Layout) -> Self {
        Self {
            version: PROFILE_VERSION,
            id: id.to_string(),
            name: name.to_string(),
            apps: Vec::new(),
            layout,
            fill_from_default: true,
            page: Page::with_slots(layout.slot_count()),
        }
    }

    pub fn is_default(&self) -> bool {
        self.id == DEFAULT_PROFILE_ID
    }

    /// Walks into nested folders. Returns `None` if the path no longer exists.
    pub fn page_at(&self, path: &[usize]) -> Option<&Page> {
        let mut page = &self.page;
        for &i in path {
            match page.slots.get(i)?.as_ref()?.action {
                Action::Folder { page: ref p } => page = p,
                _ => return None,
            }
        }
        Some(page)
    }

    /// Visits every button, including those inside folders.
    pub fn for_each_button<'a>(&'a self, f: &mut dyn FnMut(&'a Button)) {
        fn walk<'a>(page: &'a Page, f: &mut dyn FnMut(&'a Button)) {
            for b in page.slots.iter().flatten() {
                f(b);
                if let Action::Folder { page } = &b.action {
                    walk(page, f);
                }
            }
        }
        walk(&self.page, f);
    }
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MediaOp {
    PlayPause,
    Next,
    Previous,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VolumeOp {
    Up,
    Down,
    Set,
    MuteToggle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Shell {
    #[default]
    Cmd,
    Powershell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AudioFlow {
    Output,
    Input,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WindowOp {
    Minimize,
    Maximize,
    NextMonitor,
    TopmostToggle,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SystemOp {
    Lock,
    Sleep,
    Snip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ObsOp {
    SwitchScene,
    ToggleRecord,
    StartRecord,
    StopRecord,
    ToggleStream,
    StartStream,
    StopStream,
    ToggleMute,
    SaveReplay,
}

fn default_step() -> u8 {
    5
}

/// All action types (SPEC 6). Serialized with a `type` tag and camelCase fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Action {
    Hotkey {
        keys: Vec<KeyCombo>,
    },
    TypeText {
        text: String,
    },
    Launch {
        path: String,
        #[serde(default)]
        args: String,
        #[serde(default)]
        working_dir: String,
        #[serde(default)]
        admin: bool,
    },
    Open {
        target: String,
    },
    Command {
        command: String,
        #[serde(default)]
        shell: Shell,
        #[serde(default = "default_true")]
        hidden: bool,
    },
    Media {
        op: MediaOp,
    },
    NowPlaying {},
    Volume {
        op: VolumeOp,
        #[serde(default = "default_step")]
        step: u8,
        #[serde(default)]
        value: u8,
    },
    MicMute {},
    AppVolume {
        exe: String,
        op: VolumeOp,
        #[serde(default = "default_step")]
        step: u8,
    },
    AudioDevice {
        flow: AudioFlow,
        /// Device id to switch to; `None` cycles through all active devices.
        #[serde(default)]
        device_id: Option<String>,
        #[serde(default)]
        device_name: String,
    },
    Window {
        op: WindowOp,
    },
    System {
        op: SystemOp,
    },
    Obs {
        op: ObsOp,
        #[serde(default)]
        scene: String,
        #[serde(default)]
        source: String,
    },
    Multi {
        actions: Vec<Action>,
    },
    Toggle {
        a: Box<Action>,
        b: Box<Action>,
    },
    Folder {
        page: Page,
    },
    SwitchProfile {
        profile_id: String,
    },
    Delay {
        ms: u32,
    },
    Plugin {
        plugin_uuid: String,
        action_uuid: String,
    },
}

impl Action {
    pub fn type_name(&self) -> &'static str {
        match self {
            Action::Hotkey { .. } => "hotkey",
            Action::TypeText { .. } => "typeText",
            Action::Launch { .. } => "launch",
            Action::Open { .. } => "open",
            Action::Command { .. } => "command",
            Action::Media { .. } => "media",
            Action::NowPlaying {} => "nowPlaying",
            Action::Volume { .. } => "volume",
            Action::MicMute {} => "micMute",
            Action::AppVolume { .. } => "appVolume",
            Action::AudioDevice { .. } => "audioDevice",
            Action::Window { .. } => "window",
            Action::System { .. } => "system",
            Action::Obs { .. } => "obs",
            Action::Multi { .. } => "multi",
            Action::Toggle { .. } => "toggle",
            Action::Folder { .. } => "folder",
            Action::SwitchProfile { .. } => "switchProfile",
            Action::Delay { .. } => "delay",
            Action::Plugin { .. } => "plugin",
        }
    }

    pub fn hotkey(combo: KeyCombo) -> Self {
        Action::Hotkey { keys: vec![combo] }
    }
}
