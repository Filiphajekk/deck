// Pure, immutable operations on profiles used by the editor (unit-tested with Vitest).
import { newId } from "../lib/ids";
import type { Action, Button, Layout, Page, Profile } from "../types/profile";
import { slotCount } from "../types/profile";

/** Path of folder slot indices from the profile's top page. */
export type PagePath = number[];

export function getPage(profile: Profile, path: PagePath): Page | null {
  let page: Page = profile.page;
  for (const i of path) {
    const b = page.slots[i];
    if (!b || b.action.type !== "folder") return null;
    page = b.action.page;
  }
  return page;
}

function mapPage(page: Page, path: PagePath, fn: (p: Page) => Page): Page {
  if (path.length === 0) return fn(page);
  const [head, ...rest] = path;
  const b = page.slots[head];
  if (!b || b.action.type !== "folder") return page;
  const inner = mapPage(b.action.page, rest, fn);
  const slots = page.slots.slice();
  slots[head] = { ...b, action: { ...b.action, page: inner } };
  return { slots };
}

export function updatePage(profile: Profile, path: PagePath, fn: (p: Page) => Page): Profile {
  return { ...profile, page: mapPage(profile.page, path, fn) };
}

/** Slots that cannot hold buttons (grid folders use slot 0 for "Back"). */
export function isReserved(layout: Layout, path: PagePath, index: number): boolean {
  return layout.kind === "grid" && path.length > 0 && index === 0;
}

export function setSlot(
  profile: Profile,
  path: PagePath,
  index: number,
  button: Button | null,
): Profile {
  if (isReserved(profile.layout, path, index)) return profile;
  return updatePage(profile, path, (p) => {
    const slots = p.slots.slice();
    slots[index] = button;
    return { slots };
  });
}

/** Moves a button to another slot; if the target is taken the two swap places. */
export function moveSlot(profile: Profile, path: PagePath, from: number, to: number): Profile {
  if (from === to) return profile;
  if (isReserved(profile.layout, path, from) || isReserved(profile.layout, path, to))
    return profile;
  return updatePage(profile, path, (p) => {
    const slots = p.slots.slice();
    const tmp = slots[to] ?? null;
    slots[to] = slots[from] ?? null;
    slots[from] = tmp;
    return { slots };
  });
}

function reidAction(a: Action): Action {
  if (a.type === "folder") return { ...a, page: reidPage(a.page) };
  return a;
}

function reidPage(p: Page): Page {
  return { slots: p.slots.map((b) => (b ? cloneButton(b) : null)) };
}

/** Deep copy with fresh ids (also for buttons inside folders). */
export function cloneButton(b: Button): Button {
  return { ...structuredClone(b), id: newId(), action: reidAction(structuredClone(b.action)) };
}

export function firstFreeSlot(profile: Profile, path: PagePath, after = -1): number | null {
  const page = getPage(profile, path);
  if (!page) return null;
  const n = page.slots.length;
  for (let k = 1; k <= n; k++) {
    const i = (after + k + n) % n;
    if (!page.slots[i] && !isReserved(profile.layout, path, i)) return i;
  }
  return null;
}

/** Copies a button into the next free slot. Returns null when the page is full. */
export function duplicateSlot(
  profile: Profile,
  path: PagePath,
  index: number,
): { profile: Profile; index: number } | null {
  const page = getPage(profile, path);
  const b = page?.slots[index];
  if (!b) return null;
  const free = firstFreeSlot(profile, path, index);
  if (free === null) return null;
  return { profile: setSlot(profile, path, free, cloneButton(b)), index: free };
}

export function emptyPage(layout: Layout): Page {
  return { slots: Array.from({ length: slotCount(layout) }, () => null) };
}

/** Turns a slot into a folder. An existing button is moved inside the new folder. */
export function makeFolder(
  profile: Profile,
  path: PagePath,
  index: number,
  title: string,
): Profile {
  const page = getPage(profile, path);
  if (!page || isReserved(profile.layout, path, index)) return profile;
  const existing = page.slots[index];
  if (existing?.action.type === "folder") return profile;
  const inner = emptyPage(profile.layout);
  if (existing) {
    // In grid folders slot 0 is "Back", so put it into the next one.
    inner.slots[profile.layout.kind === "grid" ? 1 : 0] = existing;
  }
  const folder: Button = {
    id: newId(),
    title,
    icon: { kind: "lucide", name: "folder" },
    action: { type: "folder", page: inner },
  };
  return setSlot(profile, path, index, folder);
}

function resizePage(page: Page, n: number): { page: Page; lost: number } {
  let lost = page.slots.slice(n).filter(Boolean).length;
  const slots: (Button | null)[] = Array.from({ length: n }, (_, i) => {
    const b = page.slots[i] ?? null;
    if (b && b.action.type === "folder") {
      const r = resizePage(b.action.page, n);
      lost += r.lost;
      return { ...b, action: { ...b.action, page: r.page } };
    }
    return b;
  });
  return { page: { slots }, lost };
}

/** Changes the layout and resizes every page. `lost` counts buttons that did not fit. */
export function changeLayout(profile: Profile, layout: Layout): { profile: Profile; lost: number } {
  const r = resizePage(profile.page, slotCount(layout));
  return { profile: { ...profile, layout, page: r.page }, lost: r.lost };
}

/** Counts buttons that would be dropped by switching to `layout`. */
export function lostByLayout(profile: Profile, layout: Layout): number {
  return changeLayout(profile, layout).lost;
}

export function newProfile(
  name: string,
  apps: string[],
  layout: Layout = { kind: "radial" },
): Profile {
  return {
    version: 1,
    id: newId(),
    name,
    apps,
    layout,
    fillFromDefault: true,
    page: emptyPage(layout),
  };
}

/** Normalises an exe name the same way as the backend (`C:\x\Chrome.EXE` -> `chrome.exe`). */
export function normalizeExe(exe: string): string {
  const name = exe.split(/[\\/]/).pop() ?? exe;
  return name.trim().toLowerCase();
}
