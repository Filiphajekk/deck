# Progress

## Status
- **Phase 0 – Foundation: DONE**
- **Phase 1 – Overlay and basic actions: DONE**
- Phase 2 – Editor: next

## Phase 0
- Environment: Rust 1.100 (rustup, stable-x86_64-pc-windows-msvc, installed via winget), VS Build Tools 2022 with C++ and Windows SDK 10.0.26100, Node 24, WebView2 157.
- Tauri 2.12 + React 19 + TS 5.9 + Vite 8 + Tailwind 4 project, two frontend entries (`index.html` editor, `overlay.html` overlay).
- Tray icon (Open editor / Pause / Quit), editor window from tray, single-instance (second launch opens the editor), logging to `%APPDATA%\Deckless\logs` with panic hook.
- Verified: `npm run tauri dev` starts the app, tray created, editor window shown, second launch exits and opens editor.

## Phase 1
- Input hooks on their own thread with supervisor + watchdog; trigger hotkey (default Ctrl+Shift+Space), optional mouse button (X1/X2/middle, swallowed), hold vs tap, Esc / arrows / Enter / Backspace swallowed while open, hotkey recorder mode (used by the editor).
- Overlay window created once hidden, `WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW`, click-through, shown with SWP_NOACTIVATE; per-monitor DPI placement clamped to the work area.
- Radial (angle-based, flick works) and grid hit testing in Rust; panel drawn by the shared `PanelView` component.
- Focus restore + 30 ms delay before actions (SPEC 4.5).
- Actions implemented: hotkey, typeText, launch, open, command, media, volume, micMute, system (+ window, appVolume, audioDevice, multi, toggle, switchProfile, delay already wired; OBS / nowPlaying / plugin are stubs for phases 3–4).
- Default profile (SPEC 5.1) written to `%APPDATA%\Deckless\profiles\default.json` on first run.
- Tests: 28 Rust unit tests (profile selection by exe, slot from angle, placement, grid hit test, profile (de)serialization + migration, broken-file handling, SendInput sequences incl. partial-send cleanup, text events with diacritics/emoji, config).

### Acceptance (verified by injected input + screenshots)
- ✅ Select-all via the panel: in a plain Win32 edit control the text was fully selected (`sel=0,30`) after tap + click on a Ctrl+A button. Windows 11 Notepad opens files as tabs inside the user's existing window, so the automated run used a dedicated test window (`scripts/test-target.ps1`, RichTextBox) instead of the user's Notepad; same mechanism.
- ✅ `typeText` "Příliš žluťoučký kůň úpěl ďábelské ódy 😀" inserted exactly (hold mode, flick right, release).
- ✅ Panel opens at the cursor on both monitors (1920×1080 @125 % and 1920×1080 @100 %) and is clamped inside the work area in the corners.
- ✅ Latency from hook event to first painted frame (debug build): 7–55 ms (first open after start ~55 ms, warm opens 7–44 ms). Release build to be measured in phase 5.
- ✅ Arrow keys select the slice by direction; Esc closes; keys do not reach the app below.

## Known issues
- Input injected by *another* process (test scripts) is dropped by Windows while the editor (WebView2) window is in the foreground; real keyboard input is not affected. Tests therefore focus a different window first.
- Logging from inside the hook callback blocked when stderr was a pipe and Windows removed the hook; the callback must never log (watchdog now reinstalls hooks if that ever happens).
