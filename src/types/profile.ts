// Mirror of src-tauri/src/profiles/model.rs — keep in sync by hand.

export type Layout = { kind: "radial" } | { kind: "grid"; cols: number; rows: number };

export type Icon =
  | { kind: "lucide"; name: string }
  | { kind: "image"; src: string }
  | { kind: "emoji"; value: string };

export interface Visual {
  title: string;
  icon?: Icon | null;
}

export interface KeyCombo {
  ctrl: boolean;
  shift: boolean;
  alt: boolean;
  win: boolean;
  key: string | null;
}

export type MediaOp = "playPause" | "next" | "previous" | "stop";
export type VolumeOp = "up" | "down" | "set" | "muteToggle";
export type ShellKind = "cmd" | "powershell";
export type AudioFlow = "output" | "input";
export type WindowOp = "minimize" | "maximize" | "nextMonitor" | "topmostToggle" | "close";
export type SystemOp = "lock" | "sleep" | "snip";
export type ObsOp =
  | "switchScene"
  | "toggleRecord"
  | "startRecord"
  | "stopRecord"
  | "toggleStream"
  | "startStream"
  | "stopStream"
  | "toggleMute"
  | "saveReplay";

export type Action =
  | { type: "hotkey"; keys: KeyCombo[] }
  | { type: "typeText"; text: string }
  | { type: "launch"; path: string; args: string; workingDir: string; admin: boolean }
  | { type: "open"; target: string }
  | { type: "command"; command: string; shell: ShellKind; hidden: boolean }
  | { type: "media"; op: MediaOp }
  | { type: "nowPlaying" }
  | { type: "volume"; op: VolumeOp; step: number; value: number }
  | { type: "micMute" }
  | { type: "appVolume"; exe: string; op: VolumeOp; step: number }
  | { type: "audioDevice"; flow: AudioFlow; deviceId: string | null; deviceName: string }
  | { type: "window"; op: WindowOp }
  | { type: "system"; op: SystemOp }
  | { type: "obs"; op: ObsOp; scene: string; source: string }
  | { type: "multi"; actions: Action[] }
  | { type: "toggle"; a: Action; b: Action }
  | { type: "folder"; page: Page }
  | { type: "switchProfile"; profileId: string }
  | { type: "delay"; ms: number }
  | { type: "plugin"; pluginUuid: string; actionUuid: string };

export type ActionType = Action["type"];

export interface Button {
  id: string;
  title: string;
  icon?: Icon | null;
  stateB?: Visual | null;
  action: Action;
}

export interface Page {
  slots: (Button | null)[];
}

export interface Profile {
  version: number;
  id: string;
  name: string;
  apps: string[];
  layout: Layout;
  fillFromDefault: boolean;
  page: Page;
}

export const DEFAULT_PROFILE_ID = "default";
export const RADIAL_SLOTS = 8;

export function slotCount(layout: Layout): number {
  if (layout.kind === "radial") return RADIAL_SLOTS;
  const c = Math.min(8, Math.max(2, layout.cols));
  const r = Math.min(8, Math.max(2, layout.rows));
  return c * r;
}
