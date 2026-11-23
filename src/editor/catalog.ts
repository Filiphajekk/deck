// Action palette: presets grouped by category, each creating a button with sensible
// defaults (icon, title, second-state look).
import { newId } from "../lib/ids";
import type { TFunc } from "../i18n";
import type { Action, ActionType, Button, Visual } from "../types/profile";

export type Category = "keyboard" | "apps" | "media" | "windows" | "system" | "obs" | "plugins";

export const CATEGORIES: Category[] = [
  "keyboard",
  "apps",
  "media",
  "windows",
  "system",
  "obs",
  "plugins",
];

export interface Preset {
  id: string;
  category: Category;
  /** i18n key of the label (also the default title). */
  label: string;
  icon: string;
  make: () => Action;
  stateB?: { label: string; icon: string };
}

const mk = (p: Preset) => p;

export const PRESETS: Preset[] = [
  // Keyboard
  mk({
    id: "hotkey",
    category: "keyboard",
    label: "act.hotkey",
    icon: "keyboard",
    make: () => ({ type: "hotkey", keys: [] }),
  }),
  mk({
    id: "typeText",
    category: "keyboard",
    label: "act.typeText",
    icon: "type",
    make: () => ({ type: "typeText", text: "" }),
  }),
  mk({
    id: "multi",
    category: "keyboard",
    label: "act.multi",
    icon: "layers",
    make: () => ({ type: "multi", actions: [] }),
  }),
  // Apps
  mk({
    id: "launch",
    category: "apps",
    label: "act.launch",
    icon: "app-window",
    make: () => ({ type: "launch", path: "", args: "", workingDir: "", admin: false }),
  }),
  mk({
    id: "openUrl",
    category: "apps",
    label: "act.openUrl",
    icon: "globe",
    make: () => ({ type: "open", target: "https://" }),
  }),
  mk({
    id: "open",
    category: "apps",
    label: "act.open",
    icon: "folder-open",
    make: () => ({ type: "open", target: "" }),
  }),
  mk({
    id: "settings",
    category: "apps",
    label: "act.msSettings",
    icon: "settings",
    make: () => ({ type: "open", target: "ms-settings:" }),
  }),
  mk({
    id: "command",
    category: "apps",
    label: "act.command",
    icon: "terminal",
    make: () => ({ type: "command", command: "", shell: "cmd", hidden: true }),
  }),
  // Media & sound
  mk({
    id: "playPause",
    category: "media",
    label: "act.playPause",
    icon: "play",
    make: () => ({ type: "media", op: "playPause" }),
  }),
  mk({
    id: "next",
    category: "media",
    label: "act.next",
    icon: "skip-forward",
    make: () => ({ type: "media", op: "next" }),
  }),
  mk({
    id: "previous",
    category: "media",
    label: "act.previous",
    icon: "skip-back",
    make: () => ({ type: "media", op: "previous" }),
  }),
  mk({
    id: "nowPlaying",
    category: "media",
    label: "act.nowPlaying",
    icon: "music",
    make: () => ({ type: "nowPlaying" }),
  }),
  mk({
    id: "volUp",
    category: "media",
    label: "act.volumeUp",
    icon: "volume-2",
    make: () => ({ type: "volume", op: "up", step: 5, value: 50 }),
  }),
  mk({
    id: "volDown",
    category: "media",
    label: "act.volumeDown",
    icon: "volume-1",
    make: () => ({ type: "volume", op: "down", step: 5, value: 50 }),
  }),
  mk({
    id: "volMute",
    category: "media",
    label: "act.volumeMute",
    icon: "volume-2",
    make: () => ({ type: "volume", op: "muteToggle", step: 5, value: 50 }),
    stateB: { label: "state.muted", icon: "volume-x" },
  }),
  mk({
    id: "micMute",
    category: "media",
    label: "act.micMute",
    icon: "mic",
    make: () => ({ type: "micMute" }),
    stateB: { label: "state.muted", icon: "mic-off" },
  }),
  mk({
    id: "appVolume",
    category: "media",
    label: "act.appVolume",
    icon: "sliders-horizontal",
    make: () => ({ type: "appVolume", exe: "", op: "muteToggle", step: 10 }),
    stateB: { label: "state.muted", icon: "volume-x" },
  }),
  mk({
    id: "audioOut",
    category: "media",
    label: "act.audioOutput",
    icon: "headphones",
    make: () => ({ type: "audioDevice", flow: "output", deviceId: null, deviceName: "" }),
  }),
  mk({
    id: "audioIn",
    category: "media",
    label: "act.audioInput",
    icon: "mic-2",
    make: () => ({ type: "audioDevice", flow: "input", deviceId: null, deviceName: "" }),
  }),
  // Windows
  mk({
    id: "winMin",
    category: "windows",
    label: "act.winMinimize",
    icon: "minimize-2",
    make: () => ({ type: "window", op: "minimize" }),
  }),
  mk({
    id: "winMax",
    category: "windows",
    label: "act.winMaximize",
    icon: "maximize-2",
    make: () => ({ type: "window", op: "maximize" }),
  }),
  mk({
    id: "winMonitor",
    category: "windows",
    label: "act.winNextMonitor",
    icon: "monitor-up",
    make: () => ({ type: "window", op: "nextMonitor" }),
  }),
  mk({
    id: "winTop",
    category: "windows",
    label: "act.winTopmost",
    icon: "pin",
    make: () => ({ type: "window", op: "topmostToggle" }),
  }),
  mk({
    id: "winClose",
    category: "windows",
    label: "act.winClose",
    icon: "x",
    make: () => ({ type: "window", op: "close" }),
  }),
  // System
  mk({
    id: "lock",
    category: "system",
    label: "act.lock",
    icon: "lock",
    make: () => ({ type: "system", op: "lock" }),
  }),
  mk({
    id: "sleep",
    category: "system",
    label: "act.sleep",
    icon: "moon",
    make: () => ({ type: "system", op: "sleep" }),
  }),
  mk({
    id: "snip",
    category: "system",
    label: "act.snip",
    icon: "scissors",
    make: () => ({ type: "system", op: "snip" }),
  }),
  mk({
    id: "folder",
    category: "system",
    label: "act.folder",
    icon: "folder",
    make: () => ({ type: "folder", page: { slots: [] } }),
  }),
  mk({
    id: "toggle",
    category: "system",
    label: "act.toggle",
    icon: "toggle-left",
    make: () => ({
      type: "toggle",
      a: { type: "hotkey", keys: [] },
      b: { type: "hotkey", keys: [] },
    }),
    stateB: { label: "act.toggleB", icon: "toggle-right" },
  }),
  mk({
    id: "switchProfile",
    category: "system",
    label: "act.switchProfile",
    icon: "repeat",
    make: () => ({ type: "switchProfile", profileId: "" }),
  }),
  // OBS
  mk({
    id: "obsScene",
    category: "obs",
    label: "act.obsScene",
    icon: "clapperboard",
    make: () => ({ type: "obs", op: "switchScene", scene: "", source: "" }),
  }),
  mk({
    id: "obsRecord",
    category: "obs",
    label: "act.obsRecord",
    icon: "circle-dot",
    make: () => ({ type: "obs", op: "toggleRecord", scene: "", source: "" }),
    stateB: { label: "state.recording", icon: "circle-stop" },
  }),
  mk({
    id: "obsStream",
    category: "obs",
    label: "act.obsStream",
    icon: "radio-tower",
    make: () => ({ type: "obs", op: "toggleStream", scene: "", source: "" }),
    stateB: { label: "state.live", icon: "circle-stop" },
  }),
  mk({
    id: "obsMute",
    category: "obs",
    label: "act.obsMute",
    icon: "mic",
    make: () => ({ type: "obs", op: "toggleMute", scene: "", source: "" }),
    stateB: { label: "state.muted", icon: "mic-off" },
  }),
  mk({
    id: "obsReplay",
    category: "obs",
    label: "act.obsReplay",
    icon: "history",
    make: () => ({ type: "obs", op: "saveReplay", scene: "", source: "" }),
  }),
];

/** Action types offered inside multi actions and toggles. */
export const SUB_ACTION_TYPES: ActionType[] = [
  "hotkey",
  "typeText",
  "launch",
  "open",
  "command",
  "media",
  "volume",
  "micMute",
  "appVolume",
  "audioDevice",
  "window",
  "system",
  "obs",
  "switchProfile",
  "delay",
];

export function defaultAction(type: ActionType): Action {
  const preset = PRESETS.find((p) => p.make().type === type);
  if (preset) return preset.make();
  switch (type) {
    case "delay":
      return { type: "delay", ms: 200 };
    case "plugin":
      return { type: "plugin", pluginUuid: "", actionUuid: "" };
    default:
      return { type: "media", op: "playPause" };
  }
}

export function buttonFromPreset(p: Preset, t: TFunc): Button {
  const stateB: Visual | null = p.stateB
    ? { title: t(p.stateB.label), icon: { kind: "lucide", name: p.stateB.icon } }
    : null;
  return {
    id: newId(),
    title: t(p.label),
    icon: { kind: "lucide", name: p.icon },
    stateB,
    action: p.make(),
  };
}

/** Does this action type have a second (live) state? */
export function hasStateB(a: Action): boolean {
  switch (a.type) {
    case "micMute":
    case "toggle":
      return true;
    case "volume":
    case "appVolume":
      return a.op === "muteToggle";
    case "obs":
      return a.op !== "switchScene" && a.op !== "saveReplay";
    // Plugins provide their own state visuals (manifest states, setState / setImage).
    case "plugin":
      return false;
    default:
      return false;
  }
}
