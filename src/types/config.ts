// Mirror of src-tauri/src/config.rs.
import type { KeyCombo } from "./profile";

export type MouseTrigger = "none" | "middle" | "x1" | "x2";
export type LanguagePref = "auto" | "cs" | "en";

export interface Appearance {
  size: number;
  opacity: number;
  accent: string;
  animations: boolean;
}

export interface ObsConfig {
  enabled: boolean;
  host: string;
  port: number;
  password: string;
}

export interface Config {
  version: number;
  hotkey: KeyCombo;
  mouseTrigger: MouseTrigger;
  holdThresholdMs: number;
  appearance: Appearance;
  language: LanguagePref;
  autostart: boolean;
  runAsAdmin: boolean;
  obs: ObsConfig;
  firstRunDone: boolean;
  uipiHintShown: boolean;
}
