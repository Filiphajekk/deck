//! Media control and "now playing" through `GlobalSystemMediaTransportControlsSessionManager`.
//!
//! WinRT calls can take tens of milliseconds, so the now-playing info is refreshed on a
//! background thread and the panel only reads the cached value.

use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use base64::Engine;
use parking_lot::Mutex;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSessionManager as SessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
};
use windows::Storage::Streams::DataReader;

use crate::profiles::model::{Action, Icon};
use crate::state::DynamicVisual;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
    pub playing: bool,
    /// Album art as a PNG data URL (≤ 144 px).
    pub cover: Option<String>,
}

static CACHE: Mutex<Option<NowPlaying>> = Mutex::new(None);
static POLLER: AtomicBool = AtomicBool::new(false);

fn manager() -> anyhow::Result<SessionManager> {
    Ok(SessionManager::RequestAsync()?.join()?)
}

/// Play/pause the current media session; falls back to the media key.
pub fn play_pause() -> anyhow::Result<()> {
    let toggled = (|| -> anyhow::Result<bool> {
        let session = manager()?.GetCurrentSession()?;
        Ok(session.TryTogglePlayPauseAsync()?.join()?)
    })();
    match toggled {
        Ok(true) => Ok(()),
        _ => crate::input::send::tap_vk(0xB3),
    }
}

fn read_cover(
    session: &windows::Media::Control::GlobalSystemMediaTransportControlsSession,
) -> Option<String> {
    let props = session.TryGetMediaPropertiesAsync().ok()?.join().ok()?;
    let thumb = props.Thumbnail().ok()?;
    let stream = thumb.OpenReadAsync().ok()?.join().ok()?;
    let size = stream.Size().ok()? as u32;
    if size == 0 || size > 8 * 1024 * 1024 {
        return None;
    }
    let reader = DataReader::CreateDataReader(&stream).ok()?;
    reader.LoadAsync(size).ok()?.join().ok()?;
    let mut bytes = vec![0u8; size as usize];
    reader.ReadBytes(&mut bytes).ok()?;
    let img = image::load_from_memory(&bytes).ok()?;
    let img = crate::icons::fit(img);
    let mut png = Vec::new();
    img.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .ok()?;
    Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png)
    ))
}

/// Reads the current session (title, artist, playing state, cover).
pub fn query(previous: Option<&NowPlaying>) -> Option<NowPlaying> {
    let session = manager().ok()?.GetCurrentSession().ok()?;
    let props = session.TryGetMediaPropertiesAsync().ok()?.join().ok()?;
    let title = props.Title().map(|s| s.to_string()).unwrap_or_default();
    let artist = props.Artist().map(|s| s.to_string()).unwrap_or_default();
    let playing = session
        .GetPlaybackInfo()
        .and_then(|i| i.PlaybackStatus())
        .map(|s| s == PlaybackStatus::Playing)
        .unwrap_or(false);
    // Re-read the cover only when the track changed.
    let cover = match previous {
        Some(p) if p.title == title && p.artist == artist => p.cover.clone(),
        _ => read_cover(&session),
    };
    Some(NowPlaying {
        title,
        artist,
        playing,
        cover,
    })
}

/// Starts the background refresher (once) when a profile has a now-playing button.
pub fn ensure_poller() {
    if POLLER.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::Builder::new()
        .name("now-playing".into())
        .spawn(|| {
            crate::audio::com_init();
            loop {
                let prev = CACHE.lock().clone();
                let next = query(prev.as_ref());
                *CACHE.lock() = next;
                std::thread::sleep(Duration::from_secs(2));
            }
        })
        .ok();
}

pub fn profiles_use_now_playing() -> bool {
    let Some(core) = crate::app::try_core() else {
        return false;
    };
    let store = core.profiles.read();
    store.list().iter().any(|p| {
        let mut found = false;
        p.for_each_button(&mut |b| found |= matches!(b.action, Action::NowPlaying {}));
        found
    })
}

pub fn now_playing_visual() -> Option<DynamicVisual> {
    ensure_poller();
    let np = CACHE.lock().clone()?;
    if np.title.is_empty() {
        return None;
    }
    let title = if np.artist.is_empty() {
        np.title
    } else {
        format!("{} · {}", np.title, np.artist)
    };
    Some(DynamicVisual {
        title: Some(title),
        icon: np.cover.map(|src| Icon::Image { src }),
    })
}

/// 1 while something is playing (shown as the second state).
pub fn now_playing_state() -> u8 {
    CACHE.lock().as_ref().is_some_and(|n| n.playing) as u8
}
