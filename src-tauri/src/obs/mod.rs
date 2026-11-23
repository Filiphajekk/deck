//! obs-websocket v5 client (implemented in phase 3).

use std::collections::HashSet;

use crate::profiles::model::ObsOp;

#[derive(Debug, Clone, Default)]
pub struct ObsStatus {
    pub connected: bool,
    pub recording: bool,
    pub streaming: bool,
    pub muted_sources: HashSet<String>,
}

pub fn status() -> ObsStatus {
    ObsStatus::default()
}

pub fn run(_op: ObsOp, _scene: &str, _source: &str) -> anyhow::Result<()> {
    anyhow::bail!("OBS is not connected")
}
