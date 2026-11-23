# Decisions

Decisions made where the spec left room (one sentence each, newest at the bottom of each section).

## Phase 0
- Tauri is pinned to 2.x (crates.io already offers 3.0 alpha; the spec requires Tauri 2).
- TypeScript is pinned to 5.9 because typescript-eslint does not support TS 7 yet.
- The editor window is created on demand and destroyed on close to keep idle memory low; the overlay is created once at startup and only shown/hidden.
- Closing the editor does not quit the app; only "Quit" in the tray does.
- When launched with `--autostart` (login) the editor is not opened; a manual launch opens it.
- Data directory can be overridden with env `DECKLESS_DATA_DIR` so tests never touch the real `%APPDATA%`.
- Logs rotate daily and keep 7 files.
- App icon is an original SVG (8 segments around a centre, `assets/app-icon.svg`) rendered by `tauri icon`.
- In-app drag & drop in the editor uses pointer events, because Tauri's native file drop (needed for installing plugins) disables HTML5 drag & drop on Windows.

## Phase 1
- The overlay window is click-through and never receives input; all hit testing (angle-based radial slices, grid cells, arrows, clicks) is done in Rust from the low-level hooks, so the webview can never take focus.
- Clicks inside the panel are swallowed by the mouse hook; a click outside closes the panel and is passed through to the app below.
- Releasing any modifier of the trigger combination also ends a "hold" (users rarely release Space first).
- Before sending keystrokes, the engine waits up to 400 ms for the user to release physically held modifiers and then releases any that are still down, so Ctrl+A is not turned into Ctrl+Shift+A.
- After a swallowed trigger containing Win or Alt, an unassigned key (VK 0xE8) is injected so that Start / the menu bar do not open.
- Win+L is detected in hotkey actions and executed with `LockWorkStation`.
- Line breaks in `typeText` are sent as Enter and tabs as Tab; everything else as KEYEVENTF_UNICODE.
- Radial arrow navigation works like a d-pad: the held arrows form a direction (Up+Right = top-right slice).
- Esc closes the panel, Backspace / right click / the centre go one folder level up (or close at top level).
- In grid layout, folder pages reserve slot 0 (top-left) for "Back".
- In hold mode, releasing over a folder opens it and the panel switches to tap mode.
- Opening uses a ≤ 100 ms CSS animation; closing without an action fades out for 90 ms before the window hides; running an action hides the panel immediately (SPEC 4.5).
- Real background blur of the desktop is not possible reliably with a transparent WebView2 window (and acrylic is rectangular), so the panel uses translucency only.
- A failed action briefly (1.2 s) re-shows the panel non-interactively with a red "!" on the button; the UIPI hint is shown the same way, once.
- Elevated targets are detected (token elevation of the foreground process) and reported as a UIPI error instead of silently dropping keys.
- A hook watchdog compares `GetLastInputInfo` with the last event the hook saw and reinstalls the hooks if Windows silently removed them.
- Buttons store explicit icons/titles for both states (`stateB`); the backend only decides which state is active.
- The panel shows a curated set of ~210 Lucide icons instead of the full set to keep the overlay bundle small.
- `windows` crate 0.62 is used (same version as Tauri) to avoid duplicate builds and HWND type mismatches.
- `command` actions use `std::process::Command` (CreateProcessW underneath) with `CREATE_NO_WINDOW` when hidden.

## Phase 2
- The overlay has a small gear button (bottom-right outside the ring / top-right corner of the grid) that opens the editor (SPEC 3.2); it is hit-tested in Rust like the slots.
- Icons from executables are extracted at 256 px with `PrivateExtractIconsW` (fallback `SHGetFileInfoW`), scaled to ≤ 144 px and stored in the profile as a data URL, so the overlay never extracts icons at runtime; extraction results are cached on disk by path + mtime.
- Custom images are scaled to fit 144×144 and stored as PNG data URLs; SVG stays vector; animated GIFs keep only the first frame.
- The editor preview uses the backend's `panel_geometry` so it matches the overlay exactly, and is scaled down with CSS when the window is small.
- Empty top-level slots in app profiles show the default profile's buttons faded when "fill from default" is on.
- Clicking an action in the palette places it into the selected empty slot (or the first free one) as an alternative to dragging.
- "Try it" sends keystrokes to the window that was in the foreground before the editor (focus is restored to it first).
- Changing a layout that would drop buttons asks for confirmation and reports how many buttons are lost.
- Each profile can be exported to `<name>.deckless.json`; importing always creates a new profile with fresh ids. "Export everything" writes one JSON with config + all profiles.
- "Run as administrator" relaunches through the normal UAC prompt (`ShellExecuteEx` verb `runas`) at start; the new instance waits for the old one to exit (`--wait-pid`) so single-instance does not hand over to it. With autostart this means a UAC prompt at sign-in, which is the honest way (no UAC bypass).
- Vitest runs with its own config without Vite plugins; on this machine it must be started from PowerShell/cmd (Git Bash working-directory form breaks Vitest 5).
- All binaries (app, tests, examples) embed an app manifest with Common Controls v6 via `build.rs`, otherwise test binaries crash with STATUS_ENTRYPOINT_NOT_FOUND.

## Phase 3
- The active profile follows the foreground app continuously (WinEvent hook), not only when the panel opens; a manual `switchProfile` stays until the foreground app changes.
- Starter profiles: one "Browser" profile for all installed Chromium browsers (Chrome, Edge, Brave, Opera share the same default shortcuts) and a separate Firefox profile (private window is Ctrl+Shift+P there).
- The OBS starter profile is only offered when the OBS connection works; it gets record/stream/replay buttons plus up to five scene buttons.
- The OBS client connects lazily (first OBS action, or at start when a profile contains OBS buttons), keeps one connection, reconnects with exponential backoff (1 s → 30 s) and tracks record/stream/mute state from events for live button states.
- OBS was not installed on the build machine, so the OBS client is verified against a mock obs-websocket v5 server in an integration test (handshake with SHA-256 auth, scene switch, record toggle + RecordStateChanged event).
- "Now playing" is read on a background thread every 2 s (only when a profile has such a button); the panel just reads the cache so WinRT never delays opening.
- `audioDevice` uses `IPolicyConfig` (CLSID 870af99c-…, IID f8679f50-…, `SetDefaultEndpoint` is the 11th method) for all three roles; verified by switching the real default output and back.

## Phase 4
- The plugin host reports Stream Deck application version 6.8.0 to plugins (they compare it with their minimum version) and one virtual device `DECKLESS-VIRTUAL-0` of type 0.
- Plugins without a `UUID` in the manifest (SDK v1) take it from the folder name `<uuid>.sdPlugin`; manifests listing OS platforms without Windows are incompatible; extension-less code paths are tried as `<name>.exe`.
- Node plugins run with the system `node.exe` found on PATH (`CREATE_NO_WINDOW`); stdout/stderr go to `logs\plugin-<uuid>.log`.
- HTML plugins run in a hidden Tauri webview; Property Inspectors are shown in an iframe in the editor. Both are served by a `deckplugin://` URI scheme that injects the `connectElgatoStreamDeckSocket(...)` call (parameters URI-encoded in the query string).
- `showAlert` reuses the red "!" flash of failed actions; `showOk` and `switchToProfile` (bundled Stream Deck profiles) are logged and otherwise ignored.
- `systemDidWakeUp` is detected from a jump of the wall clock (> 30 s over a 2 s sleep) instead of a power-broadcast window.
- Two-state plugin actions toggle their state after keyUp unless the manifest sets `DisableAutomaticStates`, like the original app.
- `setImage` SVG data URLs that are not base64 are re-encoded to base64 so they work in `<img>`.
- Plugin buttons on the visible page stay "appeared" while the overlay is hidden; opening a folder in the panel switches the visible page, closing returns to the top page.
- The test plugin fixture uses the WebSocket client built into Node.js 22+, so it needs no npm packages.
- Plugin processes are assigned to a job object with KILL_ON_JOB_CLOSE so they never outlive the app.
- Phases 2–4 were developed together (shared commands/modules) and committed in one commit; the acceptance results per phase are in PROGRESS.md.
- WebView2 extra flags (`--in-process-gpu`, `--renderer-process-limit=1`) did not lower memory in a measurement, so Tauri's default browser arguments are kept.
- `README.md` is in English and links to the Czech `README.cs.md` (requested by the user; the spec asked for a Czech README, which is kept).
