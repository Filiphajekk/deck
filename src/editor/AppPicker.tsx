// List of running and installed applications with their icons (used for app profiles,
// launch actions and icons from apps).
import { AppWindow } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useT } from "../i18n";
import { api, cachedFileIcon, type AppInfo } from "../lib/api";
import { cx, TextInput } from "./ui";

function AppIcon({ path }: { path: string }) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    let alive = true;
    cachedFileIcon(path).then((s) => alive && setSrc(s));
    return () => {
      alive = false;
    };
  }, [path]);
  return src ? (
    <img src={src} alt="" className="h-6 w-6 object-contain" draggable={false} />
  ) : (
    <AppWindow size={20} className="text-neutral-500" />
  );
}

export function AppPicker({ onPick }: { onPick: (app: AppInfo) => void }) {
  const t = useT();
  const [tab, setTab] = useState<"running" | "installed">("running");
  const [running, setRunning] = useState<AppInfo[] | null>(null);
  const [installed, setInstalled] = useState<AppInfo[] | null>(null);
  const [query, setQuery] = useState("");

  useEffect(() => {
    api.runningApps().then(setRunning);
    api.installedApps().then(setInstalled);
  }, []);

  const list = tab === "running" ? running : installed;
  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!list) return null;
    return q ? list.filter((a) => a.name.toLowerCase().includes(q) || a.exe.includes(q)) : list;
  }, [list, query]);

  return (
    <div className="space-y-2">
      <div className="flex items-center gap-2">
        {(["running", "installed"] as const).map((x) => (
          <button
            key={x}
            onClick={() => setTab(x)}
            className={cx(
              "rounded-md px-3 py-1 text-sm",
              tab === x ? "bg-neutral-700 text-white" : "text-neutral-400 hover:text-white",
            )}
          >
            {t(`apps.${x}`)}
          </button>
        ))}
        <div className="flex-1" />
        <div className="w-56">
          <TextInput value={query} onChange={setQuery} placeholder={t("common.search")} />
        </div>
      </div>
      <div className="max-h-[45vh] overflow-y-auto rounded-md border border-neutral-800">
        {!filtered && <p className="p-3 text-sm text-neutral-500">{t("common.loading")}</p>}
        {filtered?.length === 0 && <p className="p-3 text-sm text-neutral-500">{t("apps.none")}</p>}
        {filtered?.map((app) => (
          <button
            key={app.path}
            onClick={() => onPick(app)}
            className="flex w-full items-center gap-3 border-b border-neutral-800/70 px-3 py-2 text-left last:border-0 hover:bg-neutral-800"
          >
            <AppIcon path={app.path} />
            <span className="min-w-0 flex-1">
              <span className="block truncate text-sm text-neutral-100">{app.name}</span>
              <span className="block truncate text-xs text-neutral-500">{app.exe}</span>
            </span>
          </button>
        ))}
      </div>
    </div>
  );
}
