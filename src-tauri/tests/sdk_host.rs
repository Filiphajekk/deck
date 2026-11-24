//! SPEC 7.6: full cycle with the test plugin (install → start → register → willAppear →
//! keyDown → setTitle → settings saved), plus persistence across a host restart.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use deckless_lib::sdk_host::messages::Instance;
use deckless_lib::sdk_host::{find_node, Host, PluginStatus};
use serde_json::Value;

const PLUGIN: &str = "com.deckless.testplugin";
const ACTION: &str = "com.deckless.testplugin.counter";
const CONTEXT: &str = "button-ctx-1";

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/com.deckless.testplugin.sdPlugin")
}

fn wait_for(what: &str, mut cond: impl FnMut() -> bool) {
    let start = Instant::now();
    while !cond() {
        assert!(
            start.elapsed() < Duration::from_secs(15),
            "timed out waiting for {what}"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn title(host: &Host) -> Option<String> {
    host.runtime_visual(CONTEXT).and_then(|(t, _, _)| t)
}

fn show_button(host: &Host) {
    host.set_visible(
        vec![(
            PLUGIN.to_string(),
            Instance {
                action: ACTION.into(),
                context: CONTEXT.into(),
                column: 1,
                row: 0,
                state: 0,
                settings: Value::Null,
            },
        )],
        (3, 3),
    );
}

#[test]
fn test_plugin_full_cycle() {
    if find_node().is_none() {
        eprintln!("Node.js not found; skipping the plugin integration test");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let plugins = tmp.path().join("plugins");
    let settings = tmp.path().join("plugin-settings");
    let logs = tmp.path().join("logs");
    std::fs::create_dir_all(&logs).unwrap();

    let host = Host::start(&plugins, &settings, &logs, None).unwrap();

    // Install (from the .sdPlugin folder) starts the plugin.
    let info = host.install(&fixture()).unwrap();
    assert_eq!(info.uuid, PLUGIN);
    assert_eq!(info.actions.len(), 1);
    assert!(info.actions[0].supported);
    assert!(info.actions[0].has_property_inspector);
    assert!(plugins
        .join("com.deckless.testplugin.sdPlugin/manifest.json")
        .is_file());

    // Registration over the WebSocket.
    wait_for("registration", || host.is_connected(PLUGIN));
    assert_eq!(host.status(PLUGIN), Some(PluginStatus::Running));

    // willAppear → plugin answers with setTitle "0".
    show_button(&host);
    wait_for("initial title", || title(&host).as_deref() == Some("0"));

    // Selecting the button = keyDown + keyUp → setTitle "1" and setSettings {count: 1}.
    host.press(PLUGIN, ACTION, CONTEXT, (1, 0)).unwrap();
    wait_for("title 1", || title(&host).as_deref() == Some("1"));
    wait_for("settings saved", || {
        host.button_settings(PLUGIN, CONTEXT)["count"] == 1
    });
    host.press(PLUGIN, ACTION, CONTEXT, (1, 0)).unwrap();
    wait_for("title 2", || title(&host).as_deref() == Some("2"));
    wait_for("settings 2", || {
        host.button_settings(PLUGIN, CONTEXT)["count"] == 2
    });

    // Settings are on disk in plugin-settings\<uuid>.json.
    let file = std::fs::read_to_string(settings.join(format!("{PLUGIN}.json"))).unwrap();
    let json: Value = serde_json::from_str(&file).unwrap();
    assert_eq!(json["buttons"][CONTEXT]["count"], 2);

    // Property Inspector URL for the editor.
    let pi = host
        .property_inspector_url(PLUGIN, ACTION, CONTEXT)
        .unwrap()
        .unwrap();
    assert!(pi.starts_with("http://deckplugin.localhost/com.deckless.testplugin/pi.html?dl="));
    host.stop_all();
    drop(host);

    // A new host (= app restart) loads the installed plugin; the count survived.
    let host2 = Host::start(&plugins, &settings, &logs, None).unwrap();
    host2.load_installed();
    wait_for("registration after restart", || host2.is_connected(PLUGIN));
    show_button(&host2);
    wait_for("restored title", || title(&host2).as_deref() == Some("2"));
    host2.press(PLUGIN, ACTION, CONTEXT, (1, 0)).unwrap();
    wait_for("title 3", || title(&host2).as_deref() == Some("3"));

    // Uninstall removes files and settings.
    host2.uninstall(PLUGIN).unwrap();
    assert!(!plugins.join("com.deckless.testplugin.sdPlugin").exists());
    assert!(!settings.join(format!("{PLUGIN}.json")).exists());
}

#[test]
fn crashing_plugin_is_marked_as_error() {
    if find_node().is_none() {
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("com.deckless.crash.sdPlugin");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(
        src.join("manifest.json"),
        r#"{"UUID":"com.deckless.crash","Name":"Crash","Version":"1","CodePath":"crash.js","Actions":[]}"#,
    )
    .unwrap();
    std::fs::write(src.join("crash.js"), "process.exit(3);").unwrap();
    let logs = tmp.path().join("logs");
    std::fs::create_dir_all(&logs).unwrap();
    let host = Host::start(
        &tmp.path().join("plugins"),
        &tmp.path().join("s"),
        &logs,
        None,
    )
    .unwrap();
    host.install(&src).unwrap();
    // 1 start + 3 restarts, then it gives up.
    wait_for("error status", || {
        matches!(
            host.status("com.deckless.crash"),
            Some(PluginStatus::Error(_))
        )
    });
}

#[test]
fn protected_elgato_manifest_gives_clear_error() {
    use std::io::Write;
    let tmp = tempfile::tempdir().unwrap();
    let archive = tmp.path().join("p.streamDeckPlugin");
    {
        let mut z = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
        let opts: zip::write::SimpleFileOptions = Default::default();
        z.start_file("com.elgato.x.sdPlugin/manifest.json", opts)
            .unwrap();
        z.write_all(b"ELGATO\x01\x00\x00\x7f\x00\x00encrypted")
            .unwrap();
        z.finish().unwrap();
    }
    let plugins = tmp.path().join("plugins");
    let logs = tmp.path().join("logs");
    std::fs::create_dir_all(&logs).unwrap();
    let host = Host::start(&plugins, &tmp.path().join("s"), &logs, None).unwrap();
    let err = host.install(&archive).unwrap_err();
    assert_eq!(format!("{err:#}"), "i18n:plugins.protected");
    // No half-installed or temporary folders are left behind.
    assert_eq!(std::fs::read_dir(&plugins).unwrap().count(), 0);

    // The real file the user tried, if it is present in the repository folder.
    let real = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../discord-2.4.0.123-mac_windows.streamDeckPlugin");
    if real.exists() {
        let err = host.install(&real).unwrap_err();
        assert_eq!(format!("{err:#}"), "i18n:plugins.protected");
    }
}

#[test]
fn reinstalling_a_running_plugin_keeps_it_connected() {
    if find_node().is_none() {
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let plugins = tmp.path().join("plugins");
    let logs = tmp.path().join("logs");
    std::fs::create_dir_all(&logs).unwrap();
    let host = Host::start(&plugins, &tmp.path().join("s"), &logs, None).unwrap();
    host.install(&fixture()).unwrap();
    wait_for("first registration", || host.is_connected(PLUGIN));
    show_button(&host);

    // Reinstall twice while it runs: the old process and its socket go away while the
    // new one registers. Before the fix the old socket closing removed the new connection
    // and an old watcher started a second copy of the plugin.
    for _ in 0..2 {
        host.install(&fixture()).unwrap();
        wait_for("registration after reinstall", || host.is_connected(PLUGIN));
    }
    // Give stale watchers/sockets time to (wrongly) act.
    std::thread::sleep(Duration::from_millis(1500));
    assert!(host.is_connected(PLUGIN), "plugin lost its connection after reinstall");
    assert_eq!(host.status(PLUGIN), Some(PluginStatus::Running));
    host.press(PLUGIN, ACTION, CONTEXT, (1, 0)).unwrap();
    wait_for("press works", || host.button_settings(PLUGIN, CONTEXT)["count"] == 1);
    host.kill_all_now();
}
