// Copies the freshly built NSIS installer to the repository root as Deckless-Setup.exe,
// so the download link in the README always points to the latest build.
import { copyFileSync, readFileSync } from "node:fs";

const conf = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8"));
const src = `src-tauri/target/release/bundle/nsis/${conf.productName}_${conf.version}_x64-setup.exe`;
copyFileSync(src, "Deckless-Setup.exe");
console.log(`Copied ${src} -> Deckless-Setup.exe`);
