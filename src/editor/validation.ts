// Form validation for actions. Returns i18n keys of the problems found.
import type { Action, Button } from "../types/profile";

export function validateAction(a: Action): string[] {
  const e: string[] = [];
  switch (a.type) {
    case "hotkey":
      if (a.keys.length === 0) e.push("val.hotkeyEmpty");
      else if (a.keys.some((k) => !k.key)) e.push("val.hotkeyIncomplete");
      break;
    case "typeText":
      if (!a.text) e.push("val.textEmpty");
      break;
    case "launch":
      if (!a.path.trim()) e.push("val.pathEmpty");
      break;
    case "open":
      if (!a.target.trim()) e.push("val.targetEmpty");
      break;
    case "command":
      if (!a.command.trim()) e.push("val.commandEmpty");
      break;
    case "volume":
      if (a.op === "set" && (a.value < 0 || a.value > 100)) e.push("val.volumeRange");
      if ((a.op === "up" || a.op === "down") && (a.step < 1 || a.step > 100))
        e.push("val.stepRange");
      break;
    case "appVolume":
      if (!a.exe.trim()) e.push("val.exeEmpty");
      if (a.op === "set") e.push("val.appVolumeSet");
      if (a.step < 1 || a.step > 100) e.push("val.stepRange");
      break;
    case "obs":
      if (a.op === "switchScene" && !a.scene.trim()) e.push("val.sceneEmpty");
      if (a.op === "toggleMute" && !a.source.trim()) e.push("val.sourceEmpty");
      break;
    case "multi":
      if (a.actions.length === 0) e.push("val.multiEmpty");
      a.actions.forEach((sub) => e.push(...validateAction(sub)));
      break;
    case "toggle":
      e.push(...validateAction(a.a), ...validateAction(a.b));
      break;
    case "switchProfile":
      if (!a.profileId) e.push("val.profileEmpty");
      break;
    case "delay":
      if (!Number.isFinite(a.ms) || a.ms < 0 || a.ms > 60000) e.push("val.delayRange");
      break;
    case "plugin":
      if (!a.pluginUuid || !a.actionUuid) e.push("val.pluginMissing");
      break;
    default:
      break;
  }
  return [...new Set(e)];
}

export function validateButton(b: Button): string[] {
  return validateAction(b.action);
}

/** Exe names for app profiles: "name.exe", no path separators. */
export function validateExeName(exe: string): string | null {
  const s = exe.trim().toLowerCase();
  if (!s) return "val.exeEmpty";
  if (/[\\/:*?"<>|]/.test(s)) return "val.exeInvalid";
  if (!s.endsWith(".exe")) return "val.exeSuffix";
  return null;
}
