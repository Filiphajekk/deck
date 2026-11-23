// Typed wrappers around Tauri commands (src-tauri/src/commands.rs).
import { invoke } from "@tauri-apps/api/core";
import type { Config } from "../types/config";
import type { PanelGeometry } from "../types/overlay";
import type { AudioFlow, Button, Layout, Profile } from "../types/profile";

export interface AppInfo {
  name: string;
  exe: string;
  path: string;
}

export interface AudioDevice {
  id: string;
  name: string;
  isDefault: boolean;
}

export const api = {
  getConfig: () => invoke<Config>("get_config"),
  setConfig: (config: Config) => invoke<void>("set_config", { config }),
  listProfiles: () => invoke<Profile[]>("list_profiles"),
  saveProfile: (profile: Profile) => invoke<void>("save_profile", { profile }),
  deleteProfile: (id: string) => invoke<void>("delete_profile", { id }),
  panelGeometry: (layout: Layout) => invoke<PanelGeometry>("panel_geometry", { layout }),
  testButton: (button: Button) => invoke<void>("test_button", { button }),
  startHotkeyRecording: () => invoke<void>("start_hotkey_recording"),
  cancelHotkeyRecording: () => invoke<void>("cancel_hotkey_recording"),
  openLogsFolder: () => invoke<void>("open_logs_folder"),
  systemLanguage: () => invoke<string>("system_language"),
  installedApps: () => invoke<AppInfo[]>("installed_apps"),
  runningApps: () => invoke<AppInfo[]>("running_apps"),
  fileIcon: (path: string) => invoke<string>("file_icon", { path }),
  loadImage: (path: string) => invoke<string>("load_image", { path }),
  audioDevices: (flow: AudioFlow) => invoke<AudioDevice[]>("audio_devices", { flow }),
  audioApps: () => invoke<string[]>("audio_apps"),
  exportProfile: (id: string, path: string) => invoke<void>("export_profile", { id, path }),
  importProfile: (path: string) => invoke<Profile>("import_profile", { path }),
  exportAll: (path: string) => invoke<void>("export_all", { path }),
  importAll: (path: string) => invoke<number>("import_all", { path }),
  isElevated: () => invoke<boolean>("is_elevated"),
  restartElevated: () => invoke<boolean>("restart_elevated"),
  latencyStats: () =>
    invoke<{ count: number; avgMs: number; maxMs: number; lastMs: number }>("latency_stats"),
};

/** Cache of exe icons so lists do not ask the backend twice. */
const iconCache = new Map<string, Promise<string | null>>();
export function cachedFileIcon(path: string): Promise<string | null> {
  let p = iconCache.get(path);
  if (!p) {
    p = api.fileIcon(path).catch(() => null);
    iconCache.set(path, p);
  }
  return p;
}
