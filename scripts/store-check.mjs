// Checks plugin store entries: finds the latest release asset (.streamDeckPlugin) on GitHub
// and reads only its manifest.json via HTTP range requests (no full download), then reports
// whether the plugin can run on Windows in Deckless.
//
//   node scripts/store-check.mjs                     # checks src-tauri/store/catalog.json
//   node scripts/store-check.mjs owner/repo ...      # checks the given repositories
import { readFileSync } from "node:fs";
import { inflateRawSync } from "node:zlib";

const UA = { "User-Agent": "deckless-store-check" };

/** Picks the Windows build when a release has several assets. Mirrors src-tauri/src/store.rs. */
export function pickAsset(assets) {
  const plugins = assets.filter((a) => /\.streamdeckplugin$/i.test(a.name));
  return (
    plugins.find((a) => /win/i.test(a.name)) ??
    plugins.find((a) => !/mac/i.test(a.name)) ??
    plugins[0] ??
    null
  );
}

async function pinnedAsset(url) {
  const res = await fetch(url, { method: "HEAD", headers: UA, redirect: "follow" });
  if (!res.ok) throw new Error(`download ${res.status}`);
  const name = decodeURIComponent(url.split("/").pop());
  const tag = decodeURIComponent(url.split("/").slice(-2)[0]);
  return {
    tag,
    prerelease: false,
    asset: { name, size: Number(res.headers.get("content-length")), browser_download_url: url },
  };
}

async function latestAsset(repo, fallbackUrl) {
  const res = await fetch(`https://api.github.com/repos/${repo}/releases?per_page=10`, { headers: UA });
  // Without a token GitHub allows 60 API calls per hour; fall back to the pinned download.
  if (!res.ok && fallbackUrl) return pinnedAsset(fallbackUrl);
  if (!res.ok) throw new Error(`GitHub API ${res.status}`);
  for (const rel of await res.json()) {
    if (rel.draft) continue;
    const a = pickAsset(rel.assets ?? []);
    if (a) return { tag: rel.tag_name, prerelease: rel.prerelease, asset: a };
  }
  return null;
}

async function range(url, start, end) {
  const res = await fetch(url, { headers: { ...UA, Range: `bytes=${start}-${end}` } });
  if (res.status !== 206 && res.status !== 200) throw new Error(`download ${res.status}`);
  const buf = Buffer.from(await res.arrayBuffer());
  // A server that ignores Range sends the whole file.
  return res.status === 200 ? buf.subarray(start, end + 1) : buf;
}

/** Reads manifest.json out of a remote ZIP using its central directory. */
async function remoteManifest(url, size) {
  const tailLen = Math.min(size, 65536 + 22);
  const tail = await range(url, size - tailLen, size - 1);
  const eocd = tail.lastIndexOf(Buffer.from([0x50, 0x4b, 0x05, 0x06]));
  if (eocd < 0) throw new Error("not a zip");
  const cdSize = tail.readUInt32LE(eocd + 12);
  const cdOffset = tail.readUInt32LE(eocd + 16);
  const cd = await range(url, cdOffset, cdOffset + cdSize - 1);
  let p = 0;
  while (p < cd.length && cd.readUInt32LE(p) === 0x02014b50) {
    const method = cd.readUInt16LE(p + 10);
    const compSize = cd.readUInt32LE(p + 20);
    const nameLen = cd.readUInt16LE(p + 28);
    const extraLen = cd.readUInt16LE(p + 30);
    const commentLen = cd.readUInt16LE(p + 32);
    const localOffset = cd.readUInt32LE(p + 42);
    const name = cd.subarray(p + 46, p + 46 + nameLen).toString("utf8");
    // Archives packed on Windows often use "\" as the path separator.
    if (/^[^/\\]+[/\\]manifest\.json$/i.test(name) || /^manifest\.json$/i.test(name)) {
      const local = await range(url, localOffset, localOffset + 30 + 1024 + compSize);
      const start = 30 + local.readUInt16LE(26) + local.readUInt16LE(28);
      const data = local.subarray(start, start + compSize);
      return { folder: name.split(/[/\\]/)[0], bytes: method === 8 ? inflateRawSync(data) : data };
    }
    p += 46 + nameLen + extraLen + commentLen;
  }
  throw new Error("no manifest.json");
}

function decode(bytes) {
  if (bytes.subarray(0, 6).toString("latin1") === "ELGATO") return null;
  if (bytes[0] === 0xff && bytes[1] === 0xfe) return bytes.subarray(2).toString("utf16le");
  return bytes.toString("utf8").replace(/^﻿/, "");
}

/** Same rules as Manifest::runtime() in src-tauri/src/sdk_host/manifest.rs. */
function windowsRuntime(m) {
  const os = m.OS ?? [];
  if (os.length && !os.some((o) => /windows/i.test(o.Platform ?? ""))) return null;
  const code = (m.CodePathWin ?? m.CodePath ?? "").trim();
  if (!code) return null;
  if (/\.html?$/i.test(code)) return "html";
  if (m.Nodejs || /\.(m|c)?js$/i.test(code)) return `node${m.Nodejs?.Version ? " " + m.Nodejs.Version : ""}`;
  // Unknown or missing extension: the Stream Deck app runs `<name>.exe` on Windows.
  return "exe";
}

export async function check(repo, fallbackUrl) {
  const rel = await latestAsset(repo, fallbackUrl);
  if (!rel) return { repo, ok: false, reason: "no .streamDeckPlugin release" };
  const { folder, bytes } = await remoteManifest(rel.asset.browser_download_url, rel.asset.size);
  const text = decode(bytes);
  if (text === null) return { repo, ok: false, reason: "encrypted manifest (Elgato)" };
  const m = JSON.parse(text);
  const runtime = windowsRuntime(m);
  const uuid = m.UUID || folder.replace(/\.sdPlugin$/i, "");
  const actions = (m.Actions ?? []).filter(
    (a) => !a.Controllers?.length || a.Controllers.some((c) => /keypad/i.test(c)),
  ).length;
  return {
    repo,
    ok: !!runtime && actions > 0,
    reason: !runtime ? "no Windows code" : actions === 0 ? "no keypad actions" : "",
    uuid,
    name: m.Name,
    version: rel.tag,
    prerelease: rel.prerelease,
    runtime,
    actions,
    sizeKb: Math.round(rel.asset.size / 1024),
    asset: rel.asset.name,
  };
}

const args = process.argv.slice(2);
// Arguments: "owner/repo" or "owner/repo=https://github.com/.../download/tag/asset".
const entries = args.length
  ? args.map((a) => a.split("="))
  : JSON.parse(readFileSync("src-tauri/store/catalog.json", "utf8")).plugins.map((p) => [p.repo, p.url]);
for (const [repo, url] of entries) {
  try {
    const r = await check(repo, url);
    console.log(
      [r.ok ? "OK" : "SKIP", repo, r.uuid ?? "", r.runtime ?? "", r.actions ?? "", r.version ?? "", r.sizeKb ?? "", r.reason]
        .join("\t"),
    );
  } catch (e) {
    console.log(["ERR", repo, "", "", "", "", "", e.message].join("\t"));
  }
}
