// Mirror of src-tauri/src/keys.rs (key names) for display.
import type { KeyCombo } from "../types/profile";

const DISPLAY: Record<string, string> = {
  Space: "Space",
  Escape: "Esc",
  Backquote: "`",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  Comma: ",",
  Period: ".",
  Slash: "/",
  Left: "←",
  Right: "→",
  Up: "↑",
  Down: "↓",
  PageUp: "PgUp",
  PageDown: "PgDn",
  Delete: "Del",
  Insert: "Ins",
  PrintScreen: "PrtSc",
  ContextMenu: "Menu",
  IntlBackslash: "\\",
  NumpadAdd: "Num +",
  NumpadSubtract: "Num −",
  NumpadMultiply: "Num *",
  NumpadDivide: "Num /",
  NumpadDecimal: "Num .",
};

export function keyLabel(key: string): string {
  if (key.startsWith("Numpad") && /^Numpad\d$/.test(key)) return `Num ${key.slice(6)}`;
  return DISPLAY[key] ?? key;
}

/** "Ctrl+Shift+T" style label; empty combo gives "". */
export function comboLabel(c: KeyCombo | null | undefined): string {
  if (!c) return "";
  const parts: string[] = [];
  if (c.ctrl) parts.push("Ctrl");
  if (c.shift) parts.push("Shift");
  if (c.alt) parts.push("Alt");
  if (c.win) parts.push("Win");
  if (c.key) parts.push(keyLabel(c.key));
  return parts.join("+");
}

export function combo(key: string, mods: Partial<Omit<KeyCombo, "key">> = {}): KeyCombo {
  return {
    ctrl: !!mods.ctrl,
    shift: !!mods.shift,
    alt: !!mods.alt,
    win: !!mods.win,
    key,
  };
}
