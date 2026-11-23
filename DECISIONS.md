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
