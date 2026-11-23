# Progress

## Status
- **Phase 0 – Foundation: DONE**
- **Phase 1 – Overlay and basic actions: DONE**
- **Phase 2 – Editor: DONE**
- **Phase 3 – App profiles and advanced actions: DONE** (real OBS not installed → verified against a mock server)
- **Phase 4 – Stream Deck plugins: DONE** (no public third-party plugin tried, see below)
- **Phase 5 – Finishing:** installer builds; installing it on this PC was not allowed in the session (needs the user) – see "Open points"

## How UI checks were run
Acceptance checks inject real mouse/keyboard input (`scripts/inject.ps1`) and read the result
from screenshots (`scripts/screenshot.ps1`) and files. They only run when the user does not use
the PC (`scripts/idle.ps1`). Test data (test profile, test plugin) was removed afterwards.

## Phase 0
- Environment: Rust 1.100 (rustup via winget), VS Build Tools 2022 (C++, Windows SDK 10.0.26100), Node 24, WebView2 157.
- Tauri 2.12 + React 19 + TS 5.9 + Vite 8 + Tailwind 4; entries `index.html` (editor), `overlay.html`.
- Tray (Open editor / Pause / Quit), single instance, logging with panic hook.

## Phase 1
- Hooks with supervisor + watchdog, hold/tap, swallowed nav keys, hotkey recorder; click-through non-activating overlay; per-monitor DPI placement; Rust hit testing; focus restore + 30 ms.
- ✅ Select-all via panel in a Win32 edit control (`sel=0,30`). Windows 11 Notepad opens files as tabs in the user's own window, so a dedicated test window (`scripts/test-target.ps1`) was used.
- ✅ typeText "Příliš žluťoučký kůň úpěl ďábelské ódy 😀" inserted exactly.
- ✅ Both monitors (1920×1080 @125 % and @100 %), clamped in the corners.
- ✅ Latency (hook event → first painted frame): **release 10–19 ms**, debug 7–55 ms.

## Phase 2
- Editor per SPEC 8 (profiles, preview, palette, inspector, icon picker incl. exe icons, grid 2–8×2–8, folders, context menu, import/export, autosave 500 ms, settings tab).
- ✅ Without touching JSON: created a profile for an app from the running-apps list, dragged "Další skladba" into a slot, changed its icon (globe), dragged it to another slot, moved it into a new folder; in the overlay the folder opens and shows the button; after an app restart everything is still there.

## Phase 3
- Profile follows the foreground app, fill from default, starter profiles + first-run wizard, actions appVolume, audioDevice, window, obs, multi, toggle, switchProfile, nowPlaying.
- ✅ Over Brave the panel shows the browser profile (new tab, reopen tab, close tab, private window, reload, history + two buttons filled from the default profile); over the shell (`explorer.exe`, desktop/taskbar) the default profile.
- ✅ Output switch Realtek speakers → MSI monitor → back, read back from Windows (`examples/audioswitch.rs`).
- ✅ OBS: scene switch, record toggle and live recording state against a mock obs-websocket v5 server (`tests/obs_mock.rs`). OBS Studio is not installed on this PC.
- Note: the user completed the wizard with an older build that did not offer starter profiles yet; the browser starter profile was then written with `examples/write_starters.rs` (same code).

## Phase 4
- ✅ `tests/sdk_host.rs`: install → start → register → willAppear → keyDown → setTitle → setSettings, survives host restart, uninstall; crashing plugin → Error after 3 restarts.
- ✅ Test plugin installed by dragging `com.deckless.testplugin.streamDeckPlugin` from Explorer onto the editor; Property Inspector loads in the editor and connects; "Counter" in the panel counted to 1 and still showed 1 after restarting the app.
- Public open-source Windows plugins were not tried: downloading third-party code into the user's machine was not done without the user. The test plugin exercises the same protocol path (Node runtime, PI, settings).
- Plugin processes are in a job object, so they die with the app even on a crash.

## Phase 5
- NSIS installer: `src-tauri/target/release/bundle/nsis/Deckless_0.1.0_x64-setup.exe` (~2.9 MB), per-user install, `.streamDeckPlugin` association, autostart via Run key (release only), app icon.
- README in Czech.

## Non-functional measurements (release build, idle, no plugins)
- Overlay latency 10–19 ms (target < 100 ms) ✅
- CPU while idle: 0 s over 10 s ✅
- Memory: deckless.exe 11 MB private; with WebView2 the whole process tree is ~169 MB private (WebView2 ~158 MB). Target "under 150 MB" is **not** met; the remainder is the WebView2 runtime baseline for the always-loaded overlay. `--in-process-gpu --renderer-process-limit=1` was tried and did not reduce it, so Tauri defaults are kept.

## SPEC checklist (summary)
See the list at the end of the session report; highlights of what is *not* done:
- Installing the built installer on this PC (needs the user to run it).
- Real OBS and third-party plugins not tried (not available / not downloaded).
- Memory target (see above).
- Blur behind the panel (translucency only, see DECISIONS).

## Open points
- Run `Deckless_0.1.0_x64-setup.exe`, check it installs and starts (tray icon), and that double-clicking a `.streamDeckPlugin` installs the plugin.

## Known issues
- Input injected by another process is dropped while the editor (WebView2) is the foreground window; real input is fine.
- Vitest must be started from PowerShell/cmd on this machine (Git Bash breaks Vitest 5).
