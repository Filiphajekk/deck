import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import en from "../i18n/en.json";
import { filterStore, STORE_CATEGORIES, type StoreItem } from "./StoreView";

const catalog = JSON.parse(
  readFileSync(join(__dirname, "../../src-tauri/store/catalog.json"), "utf8"),
).plugins as StoreItem[];

describe("plugin store", () => {
  it("every catalog category has an icon and a label", () => {
    const known = new Set(STORE_CATEGORIES.map((c) => c.id));
    for (const p of catalog) {
      expect(known.has(p.category), p.id).toBe(true);
      expect((en as Record<string, string>)[`store.cat.${p.category}`], p.id).toBeTruthy();
    }
  });

  it("filters by category and text", () => {
    expect(
      filterStore(catalog, "communication", "", "en").every((p) => p.category === "communication"),
    ).toBe(true);
    expect(filterStore(catalog, null, "teams", "cs").map((p) => p.id)).toContain("teams");
    expect(filterStore(catalog, null, "spotify", "en").map((p) => p.id)).toContain("spotify");
    expect(filterStore(catalog, null, "zzzz", "en")).toEqual([]);
  });
});
