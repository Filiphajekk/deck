// "Press a shortcut" field. Recording happens in the backend keyboard hook, so Win
// combinations and keys the webview never sees are captured too.
import { listen } from "@tauri-apps/api/event";
import { Keyboard } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useT } from "../i18n";
import { api } from "../lib/api";
import { comboLabel } from "../lib/keys";
import type { KeyCombo } from "../types/profile";
import { cx } from "./ui";

export function HotkeyInput({
  value,
  onChange,
}: {
  value: KeyCombo | null;
  onChange: (c: KeyCombo) => void;
}) {
  const t = useT();
  const [recording, setRecording] = useState(false);
  const onChangeRef = useRef(onChange);
  useEffect(() => {
    onChangeRef.current = onChange;
  }, [onChange]);

  useEffect(() => {
    if (!recording) return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    listen<KeyCombo>("hotkey:recorded", (e) => {
      setRecording(false);
      onChangeRef.current(e.payload);
    }).then((f) => {
      unlisten = f;
      if (cancelled) f();
      else void api.startHotkeyRecording();
    });
    return () => {
      cancelled = true;
      unlisten?.();
      void api.cancelHotkeyRecording();
    };
  }, [recording]);

  const label = comboLabel(value);
  return (
    <button
      type="button"
      onClick={() => setRecording((r) => !r)}
      className={cx(
        "flex w-full items-center gap-2 rounded-md border px-2.5 py-1.5 text-left text-sm",
        recording
          ? "animate-pulse border-sky-500 bg-sky-950/40 text-sky-200"
          : "border-neutral-700 bg-neutral-800 text-neutral-100 hover:border-neutral-500",
      )}
    >
      <Keyboard size={15} className="shrink-0 text-neutral-400" />
      <span className="flex-1 font-mono">
        {recording ? t("hotkey.recording") : label || t("hotkey.empty")}
      </span>
      <span className="text-xs text-neutral-500">
        {recording ? t("common.cancel") : t("hotkey.record")}
      </span>
    </button>
  );
}
