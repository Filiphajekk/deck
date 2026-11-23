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
