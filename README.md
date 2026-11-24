# Deckless

**English** · [Čeština](README.cs.md)

**A Stream Deck without the Stream Deck.** Press a shortcut (or a side mouse button) and a panel of buttons appears right at your mouse cursor. Pick an action and the panel disappears. No hardware, no phone.

- **Built-in actions:** keyboard shortcuts, typing text, launching programs, opening websites and folders, commands, media, volume (also per app), microphone, switching audio devices, windows, lock, sleep, screen snip, OBS.
- **Per-app profiles:** different buttons in your browser than in VS Code, and the panel switches on its own.
- **Stream Deck plugins:** existing plugins (Discord, Spotify, Twitch, Home Assistant…) work here too.

The app is free and runs locally only. It has no accounts and no telemetry. It only talks to localhost (OBS and plugins), and to GitHub when you install a plugin from the store.

## Installation

**Quickest:** run **[`Deckless-Setup.exe`](Deckless-Setup.exe)** from this folder (on GitHub: click it, then *Download raw file*).

**From the command line** (PowerShell):

```powershell
irm https://raw.githubusercontent.com/Filiphajekk/deck/main/install.ps1 | iex
```

Silent install without the installer window:

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/Filiphajekk/deck/main/install.ps1))) -Silent
```

Or with plain `cmd`:

```cmd
curl -L -o %TEMP%\Deckless-Setup.exe https://github.com/Filiphajekk/deck/raw/main/Deckless-Setup.exe && %TEMP%\Deckless-Setup.exe
```

After installation Deckless starts and sits in the notification area (next to the clock). By default it starts when you sign in. It installs for the current user only, so no administrator rights are needed.

The installer is not code-signed, so Windows SmartScreen may warn you. Click *More info* → *Run anyway*.

You need Windows 10 or 11 (x64) with WebView2. WebView2 comes with Windows 11; on Windows 10 the installer adds it if it is missing.

## Getting started

- **Ctrl+Shift+Space** opens the panel at the cursor.
  - **Tap it:** the panel stays open and you click a button. Close it with Esc, a click outside, or pressing the shortcut again.
  - **Hold it:** move towards a button and let go. With the radial panel a quick flick in the right direction is enough.
  - You can also move between buttons with the arrow keys and press Enter to run one.
- The gear in the panel or the tray icon opens the **editor**:
  - profiles on the left, a preview of the panel in the middle, actions on the right,
  - drag an action onto a slot; buttons can be moved around; right-click to duplicate, delete or create a folder,
  - click a button to set its title, icon and parameters on the right; **Try it** runs it right away,
  - everything is saved automatically.
- **Add profile for an app** (bottom left) creates a profile that is used while that app is in the foreground. Empty slots are filled from the default profile.
- In **Settings** you can change the shortcut, turn on a side mouse button, adjust the look and language, and set up the OBS connection.

## Stream Deck plugins

### Plugin store

The **Plugins → Store** tab has 39 hand-picked open-source plugins that work on Windows: Microsoft Teams, Google Meet, Spotify, Home Assistant, OBS, HWiNFO, Bitwarden, Docker, VS Code, JetBrains IDEs, SimHub and more. Pick a category or search, then click **Install**. The plugin is downloaded straight from its author's GitHub releases (the newest one, or a verified version if GitHub is unreachable). Each plugin has its own licence.

The catalog is `src-tauri/store/catalog.json`; `node scripts/store-check.mjs` checks every entry (release file present, readable manifest with Windows code).

### Other plugins

1. Download a plugin from the [Elgato Marketplace](https://marketplace.elgato.com/stream-deck/plugins). You get a `.streamDeckPlugin` file.
2. Install it in one of these ways:
   - double-click the file,
   - drop the file onto the editor window,
   - on the **Plugins** tab, use *Install plugin from file*.
3. The plugin's actions appear in the editor under **Plugins**. Their settings (Property Inspector) show up on the right when you click the button.

Most plugins are written in Node.js. If Node.js is not installed, the editor tells you. Get it from [nodejs.org](https://nodejs.org/) (version 20 or newer).

Actions for regular keys are supported. Actions made only for dials (Stream Deck +) are marked as unsupported.

## Known limitations

- **Games in exclusive fullscreen:** the panel cannot be drawn over them. Switch the game to borderless windowed mode.
- **Apps running as administrator:** Windows (UIPI) does not let a normal app send keystrokes into them. Turn on *Run as administrator* in Settings; Windows then asks for permission every time Deckless starts. When an action into such a window fails, Deckless reminds you once.
- **Anti-cheat:** some games ignore or block synthetic input. Deckless never tries to get around it.
- **Elgato's own plugins (e.g. Discord):** their `manifest.json` is encrypted so that only the official Stream Deck app can load them. Deckless cannot install them and never tries to get around that. Plugins from other authors are not affected.
- Stream Deck plugins that bundle their own Stream Deck profiles (`switchToProfile`) cannot use that feature. The actions themselves work.

## Data

Everything lives in `%APPDATA%\Deckless`:

```
config.json              settings
profiles\<id>.json       profiles
plugins\<uuid>.sdPlugin  installed plugins
plugin-settings\         plugin settings
icons\cache\             app icons
logs\                    logs (including plugin logs)
```

To back up everything, use *Export everything* in Settings.

## Building from source

You need:

- Rust (stable, `x86_64-pc-windows-msvc`): `winget install Rustlang.Rustup`
- Visual Studio Build Tools with "Desktop development with C++"
- Node.js 20+ (the test plugin needs 22+)

```powershell
npm install
npm run tauri dev       # development build
npm run tauri build     # NSIS installer in src-tauri\target\release\bundle\nsis
npm run installer       # the same, and copies it to Deckless-Setup.exe in the root
```

Tests and checks:

```powershell
cd src-tauri
cargo test                                   # unit + integration tests (OBS mock, plugin host)
cargo clippy --all-targets -- -D warnings
cd ..
npm test                                     # Vitest (run it from PowerShell or cmd)
npm run typecheck
npm run lint
npm run format:check
```

Architecture and decisions: [SPEC.md](SPEC.md) (Czech), [DECISIONS.md](DECISIONS.md); work status: [PROGRESS.md](PROGRESS.md).
