//! Locations of all files on disk (see SPEC 3.4).

use std::path::PathBuf;

use crate::branding::DATA_DIR_NAME;

/// `%APPDATA%\Deckless`. Can be overridden with `DECKLESS_DATA_DIR` (used by tests).
pub fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("DECKLESS_DATA_DIR") {
        return PathBuf::from(dir);
    }
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir());
    base.join(DATA_DIR_NAME)
}

pub fn config_file() -> PathBuf {
    data_dir().join("config.json")
}

pub fn profiles_dir() -> PathBuf {
    data_dir().join("profiles")
}

pub fn plugins_dir() -> PathBuf {
    data_dir().join("plugins")
}

pub fn plugin_settings_dir() -> PathBuf {
    data_dir().join("plugin-settings")
}

pub fn icon_cache_dir() -> PathBuf {
    data_dir().join("icons").join("cache")
}

pub fn logs_dir() -> PathBuf {
    data_dir().join("logs")
}

/// Creates all directories the app writes to.
pub fn ensure_dirs() -> std::io::Result<()> {
    for dir in [
        data_dir(),
        profiles_dir(),
        plugins_dir(),
        plugin_settings_dir(),
        icon_cache_dir(),
        logs_dir(),
    ] {
        std::fs::create_dir_all(dir)?;
    }
    Ok(())
}
