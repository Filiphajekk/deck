// First-run wizard: how to open the panel, starter profiles for found apps, plugin tips.
import { invoke } from "@tauri-apps/api/core";
import { Check, MousePointerClick, Puzzle, Sparkles } from "lucide-react";
import { useEffect, useState } from "react";
import { useT } from "../i18n";
import { api } from "../lib/api";
import { comboLabel } from "../lib/keys";
import type { Config } from "../types/config";
import type { Profile } from "../types/profile";
import { Btn, Modal } from "./ui";

interface Suggestions {
  profiles: Profile[];
  pluginApps: { exe: string; name: string }[];
}

export const MARKETPLACE_URL = "https://marketplace.elgato.com/stream-deck/plugins";

export function WelcomeWizard({
  config,
  onDone,
  onOpenPlugins,
}: {
  config: Config;
  onDone: (createdProfiles: Profile[]) => void;
  onOpenPlugins: () => void;
}) {
  const t = useT();
  const [step, setStep] = useState(0);
  const [sugg, setSugg] = useState<Suggestions | null>(null);
  const [chosen, setChosen] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    invoke<Suggestions>("starter_suggestions")
      .then((s) => {
        setSugg(s);
        setChosen(new Set(s.profiles.map((p) => p.id)));
      })
      .catch(() => setSugg({ profiles: [], pluginApps: [] }));
  }, []);

  const finish = async (openPlugins = false, createProfiles = true) => {
    setBusy(true);
    const created: Profile[] = [];
    for (const p of createProfiles ? (sugg?.profiles ?? []) : []) {
      if (!chosen.has(p.id)) continue;
      try {
        await api.saveProfile(p);
        created.push(p);
      } catch (e) {
        console.error(e);
      }
    }
    await api.setConfig({ ...config, firstRunDone: true }).catch(() => {});
    onDone(created);
    if (openPlugins) onOpenPlugins();
  };

  const steps = [
    <div key="0" className="space-y-4">
      <div className="flex items-center gap-3">
        <Sparkles className="text-sky-400" />
        <p className="text-sm text-neutral-300">{t("wizard.intro")}</p>
      </div>
      <div className="rounded-lg border border-neutral-700 bg-neutral-800/60 p-4 text-center">
        <p className="text-xs text-neutral-400">{t("wizard.hotkey")}</p>
        <p className="mt-1 font-mono text-2xl text-white">{comboLabel(config.hotkey)}</p>
      </div>
      <ul className="space-y-2 text-sm text-neutral-300">
        <li className="flex gap-2">
          <MousePointerClick size={18} className="shrink-0 text-neutral-400" /> {t("wizard.tap")}
        </li>
        <li className="flex gap-2">
          <MousePointerClick size={18} className="shrink-0 text-neutral-400" /> {t("wizard.hold")}
        </li>
      </ul>
      <p className="text-xs text-neutral-500">{t("wizard.changeLater")}</p>
    </div>,
    <div key="1" className="space-y-3">
      <p className="text-sm text-neutral-300">{t("wizard.starters")}</p>
      {!sugg && <p className="text-sm text-neutral-500">{t("common.loading")}</p>}
      {sugg?.profiles.length === 0 && (
        <p className="text-sm text-neutral-500">{t("wizard.noStarters")}</p>
      )}
      {sugg?.profiles.map((p) => (
        <label
          key={p.id}
          className="flex cursor-pointer items-center gap-3 rounded-lg border border-neutral-800 p-3 hover:bg-neutral-800/60"
        >
          <input
            type="checkbox"
            className="h-4 w-4 accent-sky-500"
            checked={chosen.has(p.id)}
            onChange={(e) =>
              setChosen((s) => {
                const n = new Set(s);
                if (e.target.checked) n.add(p.id);
                else n.delete(p.id);
                return n;
              })
            }
          />
          <span className="flex-1">
            <span className="block text-sm text-neutral-100">{p.name}</span>
            <span className="block text-xs text-neutral-500">
              {p.apps.join(", ")} ·{" "}
              {p.page.slots
                .filter(Boolean)
                .map((b) => b!.title)
                .join(", ")}
            </span>
          </span>
        </label>
      ))}
    </div>,
    <div key="2" className="space-y-3">
      <div className="flex items-center gap-3">
        <Puzzle className="text-sky-400" />
        <p className="text-sm text-neutral-300">{t("wizard.plugins")}</p>
      </div>
      {sugg && sugg.pluginApps.length > 0 && (
        <p className="text-sm text-neutral-200">
          {t("wizard.pluginApps", { apps: sugg.pluginApps.map((a) => a.name).join(", ") })}
        </p>
      )}
      <p className="text-xs text-neutral-500">{t("wizard.pluginsHow")}</p>
    </div>,
  ];

  const last = step === steps.length - 1;
  return (
    <Modal
      title={t(`wizard.title${step}`)}
      onClose={() => finish(false, false)}
      footer={
        <>
          {step > 0 && (
            <Btn variant="ghost" onClick={() => setStep(step - 1)}>
              {t("wizard.back")}
            </Btn>
          )}
          {last && (
            <Btn onClick={() => finish(true)} disabled={busy}>
              <Puzzle size={15} /> {t("wizard.openPlugins")}
            </Btn>
          )}
          <Btn
            variant="primary"
            onClick={() => (last ? finish() : setStep(step + 1))}
            disabled={busy}
          >
            {last ? (
              <>
                <Check size={15} /> {t("wizard.done")}
              </>
            ) : (
              t("wizard.next")
            )}
          </Btn>
        </>
      }
    >
      {steps[step]}
      <div className="mt-5 flex justify-center gap-1.5">
        {steps.map((_, i) => (
          <span
            key={i}
            className={"h-1.5 w-6 rounded-full " + (i === step ? "bg-sky-500" : "bg-neutral-700")}
          />
        ))}
      </div>
    </Modal>
  );
}
