# Deckless – notes for Claude Code

- Spec: [SPEC.md](SPEC.md) (Czech). Progress / where to continue: [PROGRESS.md](PROGRESS.md). Decisions: [DECISIONS.md](DECISIONS.md). Ideas outside the spec: [IDEAS.md](IDEAS.md).
- Work phase by phase (SPEC 11); update PROGRESS.md after each bigger step, commit after each phase.
- Windows only. Code, comments and commits in English; UI texts in `src/i18n/{cs,en}.json`.

## Toolchain
Rust is installed via rustup in `%USERPROFILE%\.cargo\bin` (add to PATH in Git Bash: `export PATH="$HOME/.cargo/bin:$PATH"`).

## Commands
- Dev run: `npm run tauri dev`
- Release build + NSIS installer: `npm run tauri build`; `npm run installer` also copies it to `Deckless-Setup.exe` in the root (commit it so the README download links stay current)
- Rust tests: `cd src-tauri && cargo test`
- Rust lint: `cd src-tauri && cargo clippy --all-targets -- -D warnings`
- Frontend: `npm test` (Vitest), `npm run typecheck`, `npm run lint`, `npm run format:check`
  - Run Vitest from PowerShell or cmd: started from Git Bash on this machine every suite fails with "Cannot read properties of undefined (reading 'config')" (path form of the working directory), while PowerShell works.
- Screenshot helper for UI checks: `powershell -File scripts/screenshot.ps1 -Out shot.png`

## Layout
- `src-tauri/src/` – Rust backend, one module per SPEC 3.1 area.
- `src/editor/` – editor window (React), `src/overlay/` – overlay panel, `src/components/` shared (PanelView), `src/types/` TS mirrors of Rust types.
- Data at runtime: `%APPDATA%\Deckless` (override with env `DECKLESS_DATA_DIR`).
