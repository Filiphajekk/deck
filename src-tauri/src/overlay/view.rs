//! What the overlay frontend receives. Mirrored in `src/types/overlay.ts`.

use serde::Serialize;

use super::geometry::PanelGeometry;
use crate::config::Appearance;
use crate::profiles::model::{Action, Button, Icon, Layout, Page};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ButtonView {
    pub id: String,
    pub title: String,
    pub icon: Option<Icon>,
    /// 0 = normal, 1 = second state (muted, recording, toggle B).
    pub state: u8,
    pub is_folder: bool,
    pub action_type: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayView {
    pub seq: u64,
    pub layout: Layout,
    pub geometry: PanelGeometry,
    pub buttons: Vec<Option<ButtonView>>,
    pub in_folder: bool,
    /// Selected slot; -1 is the centre (back/close).
    pub selected: Option<i32>,
    pub appearance: Appearance,
    pub profile_name: String,
    /// Button showing a red "!" (failed action).
    pub alert: Option<String>,
    /// Optional one-off hint key (translated by the frontend), e.g. "uipiHint".
    pub hint: Option<String>,
}

pub fn button_view(b: &Button) -> ButtonView {
    let state = crate::state::button_state(b);
    let mut title = b.title.clone();
    let mut icon = b.icon.clone();
    if state == 1 {
        if let Some(v) = &b.state_b {
            if !v.title.is_empty() {
                title = v.title.clone();
            }
            if v.icon.is_some() {
                icon = v.icon.clone();
            }
        }
    }
    if let Some(dynamic) = crate::state::dynamic_visual(b) {
        if let Some(t) = dynamic.title {
            title = t;
        }
        if let Some(i) = dynamic.icon {
            icon = Some(i);
        }
    }
    ButtonView {
        id: b.id.clone(),
        title,
        icon,
        state,
        is_folder: matches!(b.action, Action::Folder { .. }),
        action_type: b.action.type_name(),
    }
}

pub fn page_views(page: &Page) -> Vec<Option<ButtonView>> {
    page.slots
        .iter()
        .map(|s| s.as_ref().map(button_view))
        .collect()
}
