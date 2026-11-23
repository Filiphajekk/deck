// Left column: profiles (default first) and adding a profile for an application.
import { open } from "@tauri-apps/plugin-dialog";
import { AppWindow, Plus, Star, Upload } from "lucide-react";
import { useEffect, useState } from "react";
import { useT } from "../i18n";
import { api, cachedFileIcon, type AppInfo } from "../lib/api";
import { APP_NAME } from "../lib/branding";
import type { Profile } from "../types/profile";
import { AppPicker } from "./AppPicker";
import { Btn, cx, Modal } from "./ui";

function ProfileIcon({ profile, paths }: { profile: Profile; paths: Map<string, string> }) {
  const [src, setSrc] = useState<string | null>(null);
  const exePath = profile.apps.map((a) => paths.get(a)).find(Boolean);
  useEffect(() => {
    if (!exePath) return;
    let alive = true;
    cachedFileIcon(exePath).then((s) => alive && setSrc(s));
    return () => {
      alive = false;
    };
  }, [exePath]);
  if (profile.id === "default") return <Star size={16} className="text-amber-400" />;
  return src ? (
    <img src={src} alt="" className="h-4 w-4 object-contain" />
  ) : (
    <AppWindow size={16} className="text-neutral-500" />
  );
}

export function ProfilesSidebar({
  profiles,
  activeId,
  liveId,
  onSelect,
  onCreateForApp,
  onImported,
}: {
  profiles: Profile[];
  activeId: string;
  /** Profile currently active in the panel (follows the foreground app). */
  liveId: string | null;
  onSelect: (id: string) => void;
  onCreateForApp: (app: AppInfo) => void;
  onImported: (p: Profile) => void;
}) {
  const t = useT();
  const [adding, setAdding] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [exePaths, setExePaths] = useState<Map<string, string>>(new Map());

  useEffect(() => {
    // Map exe names to full paths so profiles can show their app icon.
    Promise.all([api.runningApps(), api.installedApps()]).then(([r, i]) => {
      const m = new Map<string, string>();
      [...i, ...r].forEach((a) => m.set(a.exe, a.path));
      setExePaths(m);
    });
  }, []);

  const importProfile = async () => {
    setError(null);
    const path = await open({
      multiple: false,
      filters: [{ name: APP_NAME, extensions: ["json"] }],
    });
    if (typeof path !== "string") return;
    try {
      onImported(await api.importProfile(path));
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="flex h-full flex-col">
      <div className="min-h-0 flex-1 space-y-0.5 overflow-y-auto p-2">
        {profiles.map((p) => (
          <button
            key={p.id}
            onClick={() => onSelect(p.id)}
            className={cx(
              "flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-left text-sm",
              p.id === activeId
                ? "bg-neutral-800 text-white"
                : "text-neutral-300 hover:bg-neutral-800/60",
            )}
          >
            <ProfileIcon profile={p} paths={exePaths} />
            <span className="min-w-0 flex-1">
              <span className="block truncate">{p.name}</span>
              {p.apps.length > 0 && (
                <span className="block truncate text-xs text-neutral-500">{p.apps.join(", ")}</span>
              )}
            </span>
            {p.id === liveId && (
              <span
                className="h-2 w-2 shrink-0 rounded-full bg-emerald-400"
                title={t("profile.live")}
              />
            )}
          </button>
        ))}
      </div>
      <div className="space-y-1 border-t border-neutral-800 p-2">
        {error && <p className="px-1 text-xs text-rose-400">{error}</p>}
        <Btn variant="primary" className="w-full" onClick={() => setAdding(true)}>
          <Plus size={15} /> {t("profile.addForApp")}
        </Btn>
        <Btn variant="ghost" className="w-full" onClick={importProfile}>
          <Upload size={15} /> {t("profile.import")}
        </Btn>
      </div>
      {adding && (
        <Modal title={t("profile.addForApp")} onClose={() => setAdding(false)} wide>
          <AppPicker
            onPick={(app) => {
              setAdding(false);
              onCreateForApp(app);
            }}
          />
        </Modal>
      )}
    </div>
  );
}
