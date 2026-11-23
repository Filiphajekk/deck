// Settings tab (SPEC 8): trigger, mouse button, hold threshold, appearance, language,
// autostart, run as administrator, OBS, backup and logs.
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { Download, FileText, ShieldAlert, Upload } from "lucide-react";
import { useEffect, useState } from "react";
import { useT } from "../i18n";
import { api } from "../lib/api";
import type { Config, MouseTrigger } from "../types/config";
import { HotkeyInput } from "./HotkeyInput";
import { Btn, Field, Modal, NumberInput, Section, Select, TextInput, Toggle } from "./ui";

export function SettingsTab({
  config,
  onChange,
}: {
  config: Config;
  onChange: (c: Config) => void;
}) {
  const t = useT();
  const [elevated, setElevated] = useState(false);
  const [askRestart, setAskRestart] = useState(false);
  const [obsResult, setObsResult] = useState<{ ok: boolean; msg: string } | null>(null);
  const [backupMsg, setBackupMsg] = useState<string | null>(null);
  const [latency, setLatency] = useState<{ count: number; avgMs: number; maxMs: number } | null>(
    null,
  );

  useEffect(() => {
    api.isElevated().then(setElevated);
    api.latencyStats().then(setLatency);
  }, []);

  const set = <K extends keyof Config>(k: K, v: Config[K]) => onChange({ ...config, [k]: v });
  const setAppearance = <K extends keyof Config["appearance"]>(k: K, v: Config["appearance"][K]) =>
    onChange({ ...config, appearance: { ...config.appearance, [k]: v } });
  const setObs = <K extends keyof Config["obs"]>(k: K, v: Config["obs"][K]) =>
    onChange({ ...config, obs: { ...config.obs, [k]: v } });

  const testObs = async () => {
    setObsResult(null);
    try {
      const version = await invoke<string>("obs_test", { obs: config.obs });
      setObsResult({ ok: true, msg: t("settings.obsOk", { version }) });
    } catch (e) {
      setObsResult({ ok: false, msg: String(e) });
    }
  };

  const exportAll = async () => {
    const path = await save({
      defaultPath: "deckless-backup.json",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!path) return;
    try {
      await api.exportAll(path);
      setBackupMsg(t("settings.exported"));
    } catch (e) {
      setBackupMsg(String(e));
    }
  };

  const importAll = async () => {
    const path = await open({ multiple: false, filters: [{ name: "JSON", extensions: ["json"] }] });
    if (typeof path !== "string") return;
    try {
      const n = await api.importAll(path);
      setBackupMsg(t("settings.imported", { n }));
    } catch (e) {
      setBackupMsg(String(e));
    }
  };

  return (
    <div className="h-full overflow-y-auto">
      <div className="mx-auto max-w-2xl space-y-8 p-6">
        <Section title={t("settings.trigger")}>
          <Field label={t("settings.hotkey")} hint={t("settings.hotkeyHint")}>
            <HotkeyInput value={config.hotkey} onChange={(c) => c.key && set("hotkey", c)} />
          </Field>
          <Field label={t("settings.mouse")} hint={t("settings.mouseHint")}>
            <Select<MouseTrigger>
              value={config.mouseTrigger}
              onChange={(v) => set("mouseTrigger", v)}
              options={(["none", "x1", "x2", "middle"] as const).map((v) => ({
                value: v,
                label: t(`mouse.${v}`),
              }))}
            />
          </Field>
          <Field label={t("settings.holdThreshold")} hint={t("settings.holdThresholdHint")}>
            <div className="flex items-center gap-3">
              <input
                type="range"
                min={80}
                max={800}
                step={10}
                value={config.holdThresholdMs}
                onChange={(e) => set("holdThresholdMs", Number(e.target.value))}
                className="flex-1 accent-sky-500"
              />
              <span className="w-16 text-right text-sm text-neutral-300">
                {config.holdThresholdMs} ms
              </span>
            </div>
          </Field>
        </Section>

        <Section title={t("settings.appearance")}>
          <Field label={t("settings.size")}>
            <div className="flex items-center gap-3">
              <input
                type="range"
                min={60}
                max={200}
                step={5}
                value={config.appearance.size}
                onChange={(e) => setAppearance("size", Number(e.target.value))}
                className="flex-1 accent-sky-500"
              />
              <span className="w-16 text-right text-sm text-neutral-300">
                {config.appearance.size} %
              </span>
            </div>
          </Field>
          <Field label={t("settings.opacity")}>
            <div className="flex items-center gap-3">
              <input
                type="range"
                min={30}
                max={100}
                step={1}
                value={Math.round(config.appearance.opacity * 100)}
                onChange={(e) => setAppearance("opacity", Number(e.target.value) / 100)}
                className="flex-1 accent-sky-500"
              />
              <span className="w-16 text-right text-sm text-neutral-300">
                {Math.round(config.appearance.opacity * 100)} %
              </span>
            </div>
          </Field>
          <Field label={t("settings.accent")}>
            <input
              type="color"
              value={config.appearance.accent}
              onChange={(e) => setAppearance("accent", e.target.value)}
              className="h-8 w-16 cursor-pointer rounded border border-neutral-700 bg-neutral-800"
            />
          </Field>
          <Toggle
            checked={config.appearance.animations}
            onChange={(v) => setAppearance("animations", v)}
            label={t("settings.animations")}
          />
        </Section>

        <Section title={t("settings.general")}>
          <Field label={t("settings.language")}>
            <Select
              value={config.language}
              onChange={(v) => set("language", v)}
              options={[
                { value: "auto", label: t("settings.langAuto") },
                { value: "cs", label: "Čeština" },
                { value: "en", label: "English" },
              ]}
            />
          </Field>
          <Toggle
            checked={config.autostart}
            onChange={(v) => set("autostart", v)}
            label={t("settings.autostart")}
            hint={t("settings.autostartHint")}
          />
          <Toggle
            checked={config.runAsAdmin}
            onChange={(v) => {
              set("runAsAdmin", v);
              if (v && !elevated) setAskRestart(true);
            }}
            label={t("settings.runAsAdmin")}
            hint={t("settings.runAsAdminHint")}
          />
          {elevated && (
            <p className="flex items-center gap-1.5 text-xs text-amber-400">
              <ShieldAlert size={14} /> {t("settings.isElevated")}
            </p>
          )}
        </Section>

        <Section title="OBS">
          <Toggle
            checked={config.obs.enabled}
            onChange={(v) => setObs("enabled", v)}
            label={t("settings.obsEnabled")}
            hint={t("settings.obsHint")}
          />
          <div className="grid grid-cols-3 gap-3">
            <div className="col-span-2">
              <Field label={t("settings.obsHost")}>
                <TextInput value={config.obs.host} onChange={(v) => setObs("host", v)} />
              </Field>
            </div>
            <Field label={t("settings.obsPort")}>
              <NumberInput
                min={1}
                max={65535}
                value={config.obs.port}
                onChange={(v) => Number.isFinite(v) && setObs("port", v)}
              />
            </Field>
          </div>
          <Field label={t("settings.obsPassword")}>
            <TextInput
              type="password"
              value={config.obs.password}
              onChange={(v) => setObs("password", v)}
            />
          </Field>
          <div className="flex items-center gap-3">
            <Btn onClick={testObs}>{t("settings.obsTest")}</Btn>
            {obsResult && (
              <span className={obsResult.ok ? "text-sm text-emerald-400" : "text-sm text-rose-400"}>
                {obsResult.msg}
              </span>
            )}
          </div>
        </Section>

        <Section title={t("settings.data")}>
          <div className="flex flex-wrap gap-2">
            <Btn onClick={exportAll}>
              <Download size={15} /> {t("settings.exportAll")}
            </Btn>
            <Btn onClick={importAll}>
              <Upload size={15} /> {t("settings.importAll")}
            </Btn>
            <Btn variant="ghost" onClick={() => api.openLogsFolder()}>
              <FileText size={15} /> {t("settings.logs")}
            </Btn>
          </div>
          {backupMsg && <p className="text-sm text-neutral-400">{backupMsg}</p>}
          {latency && latency.count > 0 && (
            <p className="text-xs text-neutral-500">
              {t("settings.latency", {
                avg: latency.avgMs.toFixed(0),
                max: latency.maxMs.toFixed(0),
                n: latency.count,
              })}
            </p>
          )}
        </Section>

        <Section title={t("settings.limits")}>
          <ul className="list-disc space-y-1 pl-5 text-sm text-neutral-400">
            <li>{t("limits.fullscreen")}</li>
            <li>{t("limits.admin")}</li>
            <li>{t("limits.anticheat")}</li>
          </ul>
        </Section>
      </div>

      {askRestart && (
        <Modal
          title={t("settings.restartTitle")}
          onClose={() => setAskRestart(false)}
          footer={
            <>
              <Btn variant="ghost" onClick={() => setAskRestart(false)}>
                {t("settings.restartLater")}
              </Btn>
              <Btn
                variant="primary"
                onClick={async () => {
                  setAskRestart(false);
                  // Give the debounced config save time to land before restarting.
                  await new Promise((r) => setTimeout(r, 700));
                  await api.restartElevated().catch(() => false);
                }}
              >
                {t("settings.restartNow")}
              </Btn>
            </>
          }
        >
          <p className="text-sm text-neutral-300">{t("settings.restartText")}</p>
        </Modal>
      )}
    </div>
  );
}
