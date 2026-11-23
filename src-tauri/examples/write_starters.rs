//! Writes the starter profiles for apps installed on this PC (same code as the first-run
//! wizard) into the profiles folder. Existing profiles with the same apps are skipped.
use std::collections::HashSet;

fn main() -> anyhow::Result<()> {
    let mut installed: HashSet<String> = deckless_lib::apps::installed_apps()
        .into_iter()
        .map(|a| a.exe)
        .collect();
    installed.extend(
        deckless_lib::apps::running_apps()
            .into_iter()
            .map(|a| a.exe),
    );
    let lang = deckless_lib::lang::system_lang();
    let dir = deckless_lib::paths::profiles_dir();
    let only: Option<String> = std::env::args().nth(1);
    for p in deckless_lib::profiles::starters::starter_profiles(lang, &installed, false, &[]) {
        if only.as_deref().is_some_and(|o| o != p.id) {
            continue;
        }
        let path = dir.join(format!("{}.json", p.id));
        if path.exists() {
            println!("exists: {}", p.id);
            continue;
        }
        deckless_lib::fsutil::write_json_atomic(&path, &p)?;
        println!("wrote {} for {:?}", p.id, p.apps);
    }
    Ok(())
}
