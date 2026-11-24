//! Plugin store: a curated catalog of open-source Stream Deck plugins that work on Windows.
//!
//! The catalog is bundled with the app (`store/catalog.json`), so browsing needs no network.
//! Only when the user clicks "Install" do we contact GitHub: the latest release of the
//! plugin's own repository is looked up, falling back to the verified pinned download
//! (the unauthenticated GitHub API allows only 60 requests per hour). Plugins are always
//! downloaded from their authors' releases, never re-hosted by us.

use std::io::Read;
use std::path::Path;

use anyhow::{anyhow, bail, Context};
use serde::{Deserialize, Serialize};

const CATALOG: &str = include_str!("../store/catalog.json");
const USER_AGENT: &str = concat!("Deckless/", env!("CARGO_PKG_VERSION"));
/// Largest plugin we download (the biggest in the catalog is ~50 MB).
const MAX_DOWNLOAD: u64 = 300 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Texts {
    pub cs: String,
    pub en: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreEntry {
    pub id: String,
    pub uuid: String,
    pub name: String,
    pub author: String,
    /// `owner/name` on GitHub.
    pub repo: String,
    /// Verified download of a known-good release.
    pub url: String,
    pub version: String,
    pub category: String,
    pub license: String,
    /// "exe", "node" or "html".
    pub runtime: String,
    pub description: Texts,
    #[serde(default)]
    pub requires: Option<Texts>,
}

#[derive(Debug, Deserialize)]
struct Catalog {
    plugins: Vec<StoreEntry>,
}

pub fn catalog() -> Vec<StoreEntry> {
    serde_json::from_str::<Catalog>(CATALOG)
        .map(|c| c.plugins)
        .unwrap_or_else(|e| {
            tracing::error!("bundled plugin catalog is invalid: {e}");
            Vec::new()
        })
}

pub fn entry(id: &str) -> Option<StoreEntry> {
    catalog().into_iter().find(|e| e.id == id)
}

#[derive(Debug, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Deserialize)]
struct Release {
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    assets: Vec<Asset>,
}

/// Picks the Windows build when a release has several `.streamDeckPlugin` files.
/// Mirrors `pickAsset` in scripts/store-check.mjs.
fn pick_asset(assets: &[Asset]) -> Option<&Asset> {
    let plugins: Vec<&Asset> = assets
        .iter()
        .filter(|a| a.name.to_lowercase().ends_with(".streamdeckplugin"))
        .collect();
    let lower = |a: &&Asset| a.name.to_lowercase();
    plugins
        .iter()
        .find(|a| lower(a).contains("win"))
        .or_else(|| plugins.iter().find(|a| !lower(a).contains("mac")))
        .or_else(|| plugins.first())
        .copied()
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(120)))
        .build()
        .into()
}

/// Download URL of the newest release that has a plugin file; `None` if GitHub cannot
/// tell us (rate limit, offline, no such release).
fn latest_release_url(agent: &ureq::Agent, repo: &str) -> Option<String> {
    let url = format!("https://api.github.com/repos/{repo}/releases?per_page=10");
    let mut resp = agent
        .get(&url)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/vnd.github+json")
        .call()
        .ok()?;
    let text = resp.body_mut().read_to_string().ok()?;
    let releases: Vec<Release> = serde_json::from_str(&text).ok()?;
    releases
        .iter()
        .filter(|r| !r.draft)
        .find_map(|r| pick_asset(&r.assets))
        .map(|a| a.browser_download_url.clone())
}

fn download(agent: &ureq::Agent, url: &str, dest: &Path) -> anyhow::Result<()> {
    if !url.starts_with("https://github.com/") {
        bail!("refusing to download from {url}");
    }
    let mut resp = agent
        .get(url)
        .header("User-Agent", USER_AGENT)
        .call()
        .with_context(|| format!("cannot download {url}"))?;
    let mut reader = resp.body_mut().with_config().limit(MAX_DOWNLOAD).reader();
    let mut file = std::fs::File::create(dest)?;
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut file, &buf[..n])?;
    }
    Ok(())
}

/// Downloads the plugin (latest release, or the pinned one) and installs it.
pub fn install(id: &str) -> anyhow::Result<crate::sdk_host::PluginInfo> {
    let entry = entry(id).ok_or_else(|| anyhow!("unknown store plugin {id}"))?;
    let host = crate::sdk_host::host().ok_or_else(|| anyhow!("the plugin host is not running"))?;
    install_with(host, &entry)
}

/// Downloads `entry` and installs it into `host`.
pub fn install_with(
    host: &std::sync::Arc<crate::sdk_host::Host>,
    entry: &StoreEntry,
) -> anyhow::Result<crate::sdk_host::PluginInfo> {
    let agent = agent();
    let url = latest_release_url(&agent, &entry.repo).unwrap_or_else(|| entry.url.clone());
    let file_name = url.rsplit('/').next().unwrap_or("plugin.streamDeckPlugin");
    let tmp_dir =
        std::env::temp_dir().join(format!("deckless-store-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&tmp_dir)?;
    let tmp = tmp_dir.join(sanitize(file_name));
    tracing::info!(plugin = %entry.id, %url, "downloading plugin from the store");
    let result = (|| {
        if let Err(e) = download(&agent, &url, &tmp) {
            // The latest release may be broken or gone; the pinned one was verified.
            if url != entry.url {
                tracing::warn!(
                    "download of the latest release failed ({e:#}); using the verified release"
                );
                download(&agent, &entry.url, &tmp)?;
            } else {
                return Err(e);
            }
        }
        host.install(&tmp)
    })();
    let _ = std::fs::remove_dir_all(&tmp_dir);
    result
}

fn sanitize(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "._-".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.to_lowercase().ends_with(".streamdeckplugin") {
        s
    } else {
        format!("{s}.streamDeckPlugin")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_catalog_is_valid() {
        let c = catalog();
        assert!(c.len() >= 30, "catalog has {} plugins", c.len());
        let mut ids = std::collections::HashSet::new();
        for e in &c {
            assert!(ids.insert(e.id.clone()), "duplicate id {}", e.id);
            assert!(e.url.starts_with("https://github.com/"), "{}", e.id);
            assert!(
                e.url.to_lowercase().ends_with(".streamdeckplugin"),
                "{}",
                e.id
            );
            assert!(
                !e.uuid.is_empty() && !e.description.cs.is_empty() && !e.description.en.is_empty()
            );
            assert!(
                ["exe", "node", "html"].contains(&e.runtime.as_str()),
                "{}",
                e.id
            );
        }
    }

    #[test]
    fn picks_windows_asset() {
        let a = |n: &str| Asset {
            name: n.into(),
            browser_download_url: format!("https://github.com/x/{n}"),
        };
        let assets = vec![
            a("p.mac.streamDeckPlugin"),
            a("p.streamDeckPlugin"),
            a("readme.txt"),
        ];
        assert_eq!(pick_asset(&assets).unwrap().name, "p.streamDeckPlugin");
        let assets = vec![a("p-mac.streamDeckPlugin"), a("p-win.streamDeckPlugin")];
        assert_eq!(pick_asset(&assets).unwrap().name, "p-win.streamDeckPlugin");
        assert!(pick_asset(&[a("x.zip")]).is_none());
    }

    #[test]
    fn sanitizes_file_names() {
        assert_eq!(sanitize("a b.streamDeckPlugin"), "a_b.streamDeckPlugin");
        assert_eq!(sanitize("..\\evil"), ".._evil.streamDeckPlugin");
    }
}
