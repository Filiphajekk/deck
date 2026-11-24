//! Installing plugins from `.streamDeckPlugin` archives (ZIP) or `.sdPlugin` folders.

use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context};

use super::manifest::Manifest;

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let target = to.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir(&e.path(), &target)?;
        } else {
            std::fs::copy(e.path(), target)?;
        }
    }
    Ok(())
}

/// A ZIP entry name as a relative path. Archives packed on Windows often use `\` as the
/// separator, so both are accepted. Absolute paths, drive letters and `..` are refused.
pub fn safe_relative_path(name: &str) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for part in name.split(['/', '\\']) {
        match part {
            "" | "." => continue,
            ".." => return None,
            p if p.contains(':') => return None,
            p => out.push(p),
        }
    }
    if name.starts_with(['/', '\\']) || out.as_os_str().is_empty() {
        return None;
    }
    Some(out)
}

/// Extracts a ZIP into `dest`, refusing entries that would escape it (zip slip).
fn extract_zip(archive: &Path, dest: &Path) -> anyhow::Result<()> {
    let file = std::fs::File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file).context("not a valid .streamDeckPlugin file")?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let raw = entry.name().to_string();
        let Some(rel) = safe_relative_path(&raw) else {
            bail!("the archive contains an unsafe path");
        };
        let out = dest.join(rel);
        if entry.is_dir() || raw.ends_with('\\') || raw.ends_with('/') {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut buf)?;
        std::fs::write(&out, buf)?;
    }
    Ok(())
}

/// Finds the plugin folder (the one with manifest.json) in an extracted archive.
fn find_plugin_root(dir: &Path) -> Option<PathBuf> {
    if dir.join("manifest.json").is_file() {
        return Some(dir.to_path_buf());
    }
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let p = e.path();
        if p.is_dir() && p.join("manifest.json").is_file() {
            return Some(p);
        }
    }
    None
}

/// Installs a plugin into `plugins_dir/<uuid>.sdPlugin`, replacing an older version.
/// `before_replace` is called with the uuid before files are replaced (to stop the plugin).
pub fn install(
    source: &Path,
    plugins_dir: &Path,
    before_replace: impl FnOnce(&str),
) -> anyhow::Result<Manifest> {
    let tmp = plugins_dir.join(format!(".install-{}", uuid::Uuid::new_v4().simple()));
    let result = (|| {
        std::fs::create_dir_all(&tmp)?;
        if source.is_dir() {
            let name = source.file_name().unwrap_or_default();
            copy_dir(source, &tmp.join(name))?;
        } else {
            extract_zip(source, &tmp)?;
        }
        let root = find_plugin_root(&tmp).context("the plugin has no manifest.json")?;
        let manifest = Manifest::load(&root)?;
        if manifest.uuid.contains(['\\', '/', ':']) || manifest.uuid.contains("..") {
            bail!("invalid plugin UUID");
        }
        let dest = plugins_dir.join(format!("{}.sdPlugin", manifest.uuid));
        before_replace(&manifest.uuid);
        if dest.exists() {
            std::fs::remove_dir_all(&dest).context("cannot replace the installed version")?;
        }
        std::fs::rename(&root, &dest).or_else(|_| copy_dir(&root, &dest))?;
        // Reload from the final location (relative paths resolve from there).
        Manifest::load(&dest)
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_zip(path: &Path, entries: &[(&str, &str)]) {
        let f = std::fs::File::create(path).unwrap();
        let mut z = zip::ZipWriter::new(f);
        let opts: zip::write::SimpleFileOptions = Default::default();
        for (name, body) in entries {
            z.start_file(*name, opts).unwrap();
            z.write_all(body.as_bytes()).unwrap();
        }
        z.finish().unwrap();
    }

    #[test]
    fn installs_archive_and_replaces() {
        let dir = tempfile::tempdir().unwrap();
        let plugins = dir.path().join("plugins");
        std::fs::create_dir_all(&plugins).unwrap();
        let archive = dir.path().join("x.streamDeckPlugin");
        write_zip(
            &archive,
            &[
                (
                    "com.t.p.sdPlugin/manifest.json",
                    r#"{"UUID":"com.t.p","Name":"P","Version":"1","CodePath":"a.js"}"#,
                ),
                ("com.t.p.sdPlugin/a.js", "//"),
            ],
        );
        let mut stopped = None;
        let m = install(&archive, &plugins, |u| stopped = Some(u.to_string())).unwrap();
        assert_eq!(m.uuid, "com.t.p");
        assert_eq!(stopped.as_deref(), Some("com.t.p"));
        assert!(plugins.join("com.t.p.sdPlugin/a.js").is_file());
        // Installing again replaces the folder.
        install(&archive, &plugins, |_| {}).unwrap();
        assert!(plugins.join("com.t.p.sdPlugin/manifest.json").is_file());
        // No temp folders left behind.
        assert_eq!(std::fs::read_dir(&plugins).unwrap().count(), 1);
    }

    #[test]
    fn installs_archive_with_backslash_paths() {
        let dir = tempfile::tempdir().unwrap();
        let plugins = dir.path().join("plugins");
        std::fs::create_dir_all(&plugins).unwrap();
        let archive = dir.path().join("w.streamDeckPlugin");
        write_zip(
            &archive,
            &[
                (
                    "com.t.w.sdPlugin\\manifest.json",
                    r#"{"UUID":"com.t.w","Name":"W","Version":"1","CodePath":"com.t.w"}"#,
                ),
                ("com.t.w.sdPlugin\\images\\icon.png", "x"),
            ],
        );
        let m = install(&archive, &plugins, |_| {}).unwrap();
        assert_eq!(m.uuid, "com.t.w");
        assert!(plugins
            .join("com.t.w.sdPlugin")
            .join("images")
            .join("icon.png")
            .is_file());
    }

    #[test]
    fn safe_paths() {
        assert_eq!(
            safe_relative_path("a\\b/c.txt"),
            Some(PathBuf::from("a").join("b").join("c.txt"))
        );
        assert_eq!(safe_relative_path("..\\evil"), None);
        assert_eq!(safe_relative_path("a/../../evil"), None);
        assert_eq!(safe_relative_path("C:\\Windows\\x"), None);
        assert_eq!(safe_relative_path("\\abs"), None);
        assert_eq!(safe_relative_path("/abs"), None);
    }

    #[test]
    fn rejects_zip_slip() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("bad.streamDeckPlugin");
        write_zip(&archive, &[("../evil.txt", "x")]);
        assert!(install(&archive, dir.path(), |_| {}).is_err());
        assert!(!dir.path().parent().unwrap().join("evil.txt").exists());
    }
}
