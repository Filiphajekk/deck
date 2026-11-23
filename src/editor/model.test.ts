import { describe, expect, it } from "vitest";
import type { Button, Profile } from "../types/profile";
import {
  changeLayout,
  duplicateSlot,
  getPage,
  makeFolder,
  moveSlot,
  newProfile,
  normalizeExe,
  setSlot,
} from "./model";
import { validateAction, validateExeName } from "./validation";

function btn(title: string): Button {
  return { id: title, title, action: { type: "media", op: "next" } };
}

function profile(): Profile {
  let p = newProfile("Test", []);
  p = setSlot(p, [], 0, btn("a"));
  p = setSlot(p, [], 1, btn("b"));
  return p;
}

describe("slot moving", () => {
  it("moves into an empty slot", () => {
    const p = moveSlot(profile(), [], 0, 5);
    expect(p.page.slots[0]).toBeNull();
    expect(p.page.slots[5]?.title).toBe("a");
  });

  it("swaps with an occupied slot", () => {
    const p = moveSlot(profile(), [], 0, 1);
    expect(p.page.slots[0]?.title).toBe("b");
    expect(p.page.slots[1]?.title).toBe("a");
  });

  it("does not mutate the original", () => {
    const orig = profile();
    moveSlot(orig, [], 0, 3);
    expect(orig.page.slots[0]?.title).toBe("a");
  });

  it("works inside folders", () => {
    let p = makeFolder(profile(), [], 2, "Folder");
    p = setSlot(p, [2], 4, btn("inner"));
    p = moveSlot(p, [2], 4, 7);
    expect(getPage(p, [2])?.slots[7]?.title).toBe("inner");
    expect(getPage(p, [2])?.slots[4]).toBeNull();
    // The top page is untouched.
    expect(p.page.slots[0]?.title).toBe("a");
  });

  it("never moves the reserved back slot in grid folders", () => {
    let p = newProfile("G", [], { kind: "grid", cols: 3, rows: 3 });
    p = makeFolder(p, [], 0, "F");
    p = setSlot(p, [0], 2, btn("x"));
    const moved = moveSlot(p, [0], 2, 0);
    expect(getPage(moved, [0])?.slots[0]).toBeNull();
    expect(getPage(moved, [0])?.slots[2]?.title).toBe("x");
  });
});

describe("folders and duplication", () => {
  it("wraps an existing button into a new folder", () => {
    const p = makeFolder(profile(), [], 0, "F");
    const folder = p.page.slots[0];
    expect(folder?.action.type).toBe("folder");
    expect(getPage(p, [0])?.slots[0]?.title).toBe("a");
  });

  it("duplicates into the next free slot with new ids", () => {
    const r = duplicateSlot(profile(), [], 0);
    expect(r?.index).toBe(2);
    expect(r?.profile.page.slots[2]?.title).toBe("a");
    expect(r?.profile.page.slots[2]?.id).not.toBe("a");
  });

  it("returns null when the page is full", () => {
    let p = profile();
    for (let i = 0; i < 8; i++) p = setSlot(p, [], i, btn(`b${i}`));
    expect(duplicateSlot(p, [], 0)).toBeNull();
  });
});

describe("layout change", () => {
  it("pads and reports lost buttons", () => {
    const grid = changeLayout(profile(), { kind: "grid", cols: 4, rows: 4 });
    expect(grid.profile.page.slots).toHaveLength(16);
    expect(grid.lost).toBe(0);
    let p = grid.profile;
    p = setSlot(p, [], 15, btn("last"));
    const back = changeLayout(p, { kind: "radial" });
    expect(back.profile.page.slots).toHaveLength(8);
    expect(back.lost).toBe(1);
  });
});

describe("validation", () => {
  it("flags incomplete actions", () => {
    expect(validateAction({ type: "hotkey", keys: [] })).toContain("val.hotkeyEmpty");
    expect(
      validateAction({
        type: "hotkey",
        keys: [{ ctrl: true, shift: false, alt: false, win: false, key: null }],
      }),
    ).toContain("val.hotkeyIncomplete");
    expect(
      validateAction({ type: "launch", path: " ", args: "", workingDir: "", admin: false }),
    ).toContain("val.pathEmpty");
    expect(validateAction({ type: "delay", ms: 70000 })).toContain("val.delayRange");
    expect(validateAction({ type: "volume", op: "set", step: 5, value: 120 })).toContain(
      "val.volumeRange",
    );
  });

  it("validates nested actions", () => {
    const errs = validateAction({
      type: "multi",
      actions: [
        { type: "typeText", text: "" },
        { type: "delay", ms: 10 },
      ],
    });
    expect(errs).toEqual(["val.textEmpty"]);
  });

  it("accepts valid actions", () => {
    expect(validateAction({ type: "media", op: "next" })).toEqual([]);
    expect(validateAction({ type: "open", target: "https://example.com" })).toEqual([]);
  });

  it("validates exe names", () => {
    expect(validateExeName("chrome.exe")).toBeNull();
    expect(validateExeName("")).toBe("val.exeEmpty");
    expect(validateExeName("C:\\x\\a.exe")).toBe("val.exeInvalid");
    expect(validateExeName("chrome")).toBe("val.exeSuffix");
    expect(normalizeExe("C:\\Program Files\\Chrome.EXE")).toBe("chrome.exe");
  });
});
