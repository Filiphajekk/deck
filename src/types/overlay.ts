// Mirror of src-tauri/src/overlay/{view,geometry}.rs.
import type { Appearance } from "./config";
import type { Icon, Layout } from "./profile";

export interface PanelGeometry {
  width: number;
  height: number;
  outer: number;
  inner: number;
  cell: number;
  gap: number;
  pad: number;
  cols: number;
  rows: number;
  margin: number;
}

/** What one slot shows (resolved title/icon for the current state). */
export interface ButtonView {
  id: string;
  title: string;
  icon?: Icon | null;
  state: number;
  isFolder: boolean;
  actionType: string;
}

export interface OverlayView {
  seq: number;
  layout: Layout;
  geometry: PanelGeometry;
  buttons: (ButtonView | null)[];
  inFolder: boolean;
  selected: number | null;
  appearance: Appearance;
  profileName: string;
  alert: string | null;
  hint: string | null;
}
