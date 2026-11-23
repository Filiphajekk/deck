// Profile bar above the preview: name, apps, layout, fill from default, import/export/delete.
import { save } from "@tauri-apps/plugin-dialog";
import { Download, Plus, Trash2, X } from "lucide-react";
import { useState } from "react";
import { useT } from "../i18n";
import { api } from "../lib/api";
import { APP_NAME, PROFILE_EXPORT_EXT } from "../lib/branding";
import type { Layout, Profile } from "../types/profile";
import { AppPicker } from "./AppPicker";
import { changeLayout, lostByLayout, normalizeExe } from "./model";
import { validateExeName } from "./validation";
import { Btn, Modal, NumberInput, Select, TextInput, Toggle } from "./ui";

export function ProfileSettings({
  profile,
  onChange,
  onDelete,
}: {
  profile: Profile;
  onChange: (p: Profile) => void;
  onDelete: () => void;
}) {
  const t = useT();
  const [appDialog, setAppDialog] = useState(false);
  const [manualExe, setManualExe] = useState("");
  const [pendingLayout, setPendingLayout] = useState<{ layout: Layout; lost: number } | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const isDefault = profile.id === "default";

  const applyLayout = (layout: Layout) => {
    const lost = lostByLayout(profile, layout);
    if (lost > 0) setPendingLayout({ layout, lost });
    else onChange(changeLayout(profile, layout).profile);
  };

  const addExe = (exe: string) => {
    const e = normalizeExe(exe);
    if (validateExeName(e) || profile.apps.includes(e)) return;
    onChange({ ...profile, apps: [...profile.apps, e] });
  };

  const exportProfile = async () => {
    const path = await save({
      defaultPath: `${profile.name}.${PROFILE_EXPORT_EXT}`,
      filters: [{ name: APP_NAME, extensions: [PROFILE_EXPORT_EXT.split(".").pop()!] }],
    });
    if (!path) return;
    try {
      await api.exportProfile(profile.id, path);
      setMessage(t("profile.exported"));
    } catch (e) {
      setMessage(String(e));
    }
  };

  const grid = profile.layout.kind === "grid" ? profile.layout : null;
  const exeError = manualExe ? validateExeName(manualExe) : null;

  return (
    <div className="space-y-3 border-b border-neutral-800 px-4 py-3">
      <div className="flex flex-wrap items-end gap-3">
        <label className="min-w-48 flex-1">
          <span className="mb-1 block text-xs text-neutral-400">{t("profile.name")}</span>
          <TextInput value={profile.name} onChange={(name) => onChange({ ...profile, name })} />
        </label>
        <label>
          <span className="mb-1 block text-xs text-neutral-400">{t("profile.layout")}</span>
          <Select
            value={profile.layout.kind}
            onChange={(kind) =>
              applyLayout(
                kind === "radial" ? { kind: "radial" } : { kind: "grid", cols: 4, rows: 3 },
              )
            }
            options={[
              { value: "radial", label: t("layout.radial") },
              { value: "grid", label: t("layout.grid") },
            ]}
          />
        </label>
        {grid && (
          <>
            <label className="w-20">
              <span className="mb-1 block text-xs text-neutral-400">{t("layout.cols")}</span>
              <NumberInput
                min={2}
                max={8}
                value={grid.cols}
                onChange={(cols) =>
                  Number.isFinite(cols) &&
                  applyLayout({ ...grid, cols: Math.min(8, Math.max(2, cols)) })
                }
              />
            </label>
            <label className="w-20">
              <span className="mb-1 block text-xs text-neutral-400">{t("layout.rows")}</span>
              <NumberInput
                min={2}
                max={8}
                value={grid.rows}
                onChange={(rows) =>
                  Number.isFinite(rows) &&
                  applyLayout({ ...grid, rows: Math.min(8, Math.max(2, rows)) })
                }
              />
            </label>
          </>
        )}
        <Btn variant="ghost" onClick={exportProfile} title={t("profile.export")}>
          <Download size={15} /> {t("profile.export")}
        </Btn>
        {!isDefault && (
          <Btn
            variant="ghost"
            className="text-rose-400"
            onClick={onDelete}
            title={t("profile.delete")}
          >
            <Trash2 size={15} />
          </Btn>
        )}
      </div>

      {!isDefault && (
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-xs text-neutral-400">{t("profile.apps")}</span>
          {profile.apps.map((a) => (
            <span
              key={a}
              className="flex items-center gap-1 rounded-full bg-neutral-800 py-0.5 pr-1 pl-2.5 text-xs text-neutral-200"
            >
              {a}
              <button
                className="text-neutral-500 hover:text-rose-400"
                onClick={() => onChange({ ...profile, apps: profile.apps.filter((x) => x !== a) })}
              >
                <X size={12} />
              </button>
            </span>
          ))}
          <Btn small variant="ghost" onClick={() => setAppDialog(true)}>
            <Plus size={13} /> {t("profile.addApp")}
          </Btn>
          <div className="flex-1" />
          <Toggle
            checked={profile.fillFromDefault}
            onChange={(fillFromDefault) => onChange({ ...profile, fillFromDefault })}
            label={t("profile.fill")}
          />
        </div>
      )}
      {message && <p className="text-xs text-neutral-400">{message}</p>}

      {appDialog && (
        <Modal title={t("profile.addApp")} onClose={() => setAppDialog(false)} wide>
          <div className="mb-3 flex items-start gap-2">
            <div className="flex-1">
              <TextInput value={manualExe} onChange={setManualExe} placeholder="game.exe" />
              {exeError && <p className="mt-1 text-xs text-rose-400">{t(exeError)}</p>}
            </div>
            <Btn
              variant="primary"
              disabled={!manualExe || !!exeError}
              onClick={() => {
                addExe(manualExe);
                setManualExe("");
                setAppDialog(false);
              }}
            >
              {t("common.add")}
            </Btn>
          </div>
          <AppPicker
            onPick={(app) => {
              addExe(app.exe);
              setAppDialog(false);
            }}
          />
        </Modal>
      )}

      {pendingLayout && (
        <Modal
          title={t("layout.confirmTitle")}
          onClose={() => setPendingLayout(null)}
          footer={
            <>
              <Btn variant="ghost" onClick={() => setPendingLayout(null)}>
                {t("common.cancel")}
              </Btn>
              <Btn
                variant="danger"
                onClick={() => {
                  onChange(changeLayout(profile, pendingLayout.layout).profile);
                  setPendingLayout(null);
                }}
              >
                {t("layout.confirm")}
              </Btn>
            </>
          }
        >
          <p className="text-sm text-neutral-300">{t("layout.lost", { n: pendingLayout.lost })}</p>
        </Modal>
      )}
    </div>
  );
}
