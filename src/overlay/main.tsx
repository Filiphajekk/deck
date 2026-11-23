// Overlay window: renders the panel pushed by the backend. It never handles input itself;
// the backend hit-tests via the input hooks and only sends selection updates.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { flushSync } from "react-dom";
import "../index.css";
import { PanelView } from "../components/PanelView";
import { resolveLang, translate, type Lang } from "../i18n";
import type { Config } from "../types/config";
import type { OverlayView } from "../types/overlay";

type Phase = "hidden" | "open" | "closing";

function Overlay() {
  const [view, setView] = useState<OverlayView | null>(null);
  const [selected, setSelected] = useState<number | null>(null);
  const [phase, setPhase] = useState<Phase>("hidden");
  const [lang, setLang] = useState<Lang>("en");

  useEffect(() => {
    let sysLang = "en";
    const applyConfig = (c: Config) => setLang(resolveLang(c.language, sysLang));
    invoke<string>("system_language").then((l) => {
      sysLang = l;
      invoke<Config>("get_config").then(applyConfig);
    });
    let closeTimer: number | undefined;
    const unsubs = [
      listen<OverlayView>("overlay:show", (e) => {
        window.clearTimeout(closeTimer);
        // Render synchronously so the frame after this event already shows the panel.
        flushSync(() => {
          setView(e.payload);
          setSelected(e.payload.selected);
          setPhase("open");
        });
        const seq = e.payload.seq;
        requestAnimationFrame(() => invoke("overlay_rendered", { seq }));
      }),
      listen<{ seq: number; selected: number | null }>("overlay:select", (e) => {
        setSelected(e.payload.selected);
      }),
      listen<{ animate: boolean }>("overlay:hide", (e) => {
        if (e.payload?.animate) {
          setPhase("closing");
          closeTimer = window.setTimeout(() => setPhase("hidden"), 100);
        } else {
          setPhase("hidden");
        }
      }),
      listen<Config>("config:changed", (e) => applyConfig(e.payload)),
    ];
    return () => {
      unsubs.forEach((u) => u.then((f) => f()));
    };
  }, []);

  if (phase === "hidden" || !view) return null;
  const animate = view.appearance.animations;
  const cls = !animate ? "" : phase === "closing" ? "panel-anim-out" : "panel-anim-in";
  const t = (k: string) => translate(lang, k);
  return (
    <div className={cls} style={{ width: view.geometry.width, height: view.geometry.height }}>
      <PanelView
        layout={view.layout}
        geometry={view.geometry}
        buttons={view.buttons}
        inFolder={view.inFolder}
        selected={selected}
        appearance={view.appearance}
        alert={view.alert}
        backLabel={t("overlay.back")}
        closeLabel={t("overlay.close")}
        showGear
      />
      {view.hint && (
        <div className="absolute inset-x-6 bottom-1/2 translate-y-1/2 rounded-lg bg-black/85 p-3 text-center text-sm leading-snug text-white shadow-lg">
          {t(`overlay.${view.hint}`)}
        </div>
      )}
    </div>
  );
}

createRoot(document.getElementById("root")!).render(<Overlay />);
