import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import cs from "./cs.json";
import en from "./en.json";

function sources(dir: string): string[] {
  return readdirSync(dir).flatMap((f) => {
    const p = join(dir, f);
    if (statSync(p).isDirectory()) return sources(p);
    return /\.(tsx?|ts)$/.test(f) && !f.endsWith(".test.ts") ? [p] : [];
  });
}

describe("i18n", () => {
  it("Czech and English have the same keys", () => {
    expect(Object.keys(cs).sort()).toEqual(Object.keys(en).sort());
  });

  it("every literal key used in the code exists", () => {
    const keys = new Set(Object.keys(en));
    const used = new Set<string>();
    for (const file of sources(join(__dirname, ".."))) {
      const text = readFileSync(file, "utf8");
      for (const m of text.matchAll(/\bt\(\s*"([a-zA-Z0-9_.]+)"/g)) used.add(m[1]);
      for (const m of text.matchAll(/"((?:val|act|state)\.[a-zA-Z]+)"/g)) used.add(m[1]);
    }
    const missing = [...used].filter((k) => !keys.has(k));
    expect(missing).toEqual([]);
  });
});
