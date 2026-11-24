//! Installs a real plugin from the store catalog over the network (GitHub). Ignored by
//! default; run with `cargo test --test store_online -- --ignored`.

use deckless_lib::sdk_host::Host;

#[test]
#[ignore = "needs internet access"]
fn installs_small_plugin_from_github() {
    let tmp = tempfile::tempdir().unwrap();
    let logs = tmp.path().join("logs");
    std::fs::create_dir_all(&logs).unwrap();
    let plugins = tmp.path().join("plugins");
    // No window system in tests, so the (HTML) plugin is installed but not started.
    let host = Host::start(&plugins, &tmp.path().join("s"), &logs, None).unwrap();
    let entry = deckless_lib::store::entry("adguard").expect("adguard in catalog");
    let info = deckless_lib::store::install_with(&host, &entry).unwrap();
    assert_eq!(info.uuid, entry.uuid);
    assert!(!info.actions.is_empty());
    assert!(plugins
        .join(format!("{}.sdPlugin", entry.uuid))
        .join("manifest.json")
        .is_file());
}
