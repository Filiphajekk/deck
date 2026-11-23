// Editor window: profiles (left), panel preview (centre), actions / button inspector (right),
// plus Settings and Plugins tabs. Every change is saved automatically after 500 ms.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { LayoutGrid, Puzzle, Settings } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { I18nProvider, resolveLang, useT, type Lang } from "../i18n";
import { api, type AppInfo } from "../lib/api";
import { APP_NAME } from "../lib/branding";
import type { Config } from "../types/config";
import type { Button, Profile } from "../types/profile";
import { ActionPalette, presetItems, type PaletteItem } from "./ActionPalette";
import { ButtonInspector } from "./ButtonInspector";
import { buttonFromPreset, PRESETS } from "./catalog";
import { DndProvider, type DragPayload } from "./dnd";
import {
  duplicateSlot,
  emptyPage,
  getPage,
  isReserved,
  makeFolder,
  moveSlot,
  newProfile,
  setSlot,
  type PagePath,
} from "./model";
import { PanelEditor } from "./PanelEditor";
import { ProfileSettings } from "./ProfileSettings";
import { ProfilesSidebar } from "./ProfilesSidebar";
import { SettingsTab } from "./SettingsTab";
import { cx } from "./ui";
import { WelcomeWizard } from "./WelcomeWizard";
import { PluginsTab, usePluginPalette, PluginInspector } from "./plugins";

type Tab = "profiles" | "settings" | "plugins";

const SAVE_DELAY = 500;

function initialTab(): Tab {
  const h = window.location.hash.replace("#", "");
  return h === "settings" || h === "plugins" ? h : "profiles";
}

export function App() {
  const [config, setConfig] = useState<Config | null>(null);
  const [sysLang, setSysLang] = useState("en");

  useEffect(() => {
    api.systemLanguage().then(setSysLang);
    api.getConfig().then(setConfig);
  }, []);

  if (!config) return <div className="h-full bg-neutral-900" />;
  const lang: Lang = resolveLang(config.language, sysLang);
  return (
    <I18nProvider lang={lang}>
      <Editor config={config} setConfig={setConfig} />
    </I18nProvider>
  );
}

function useDebounced<T>(save: (v: T) => Promise<unknown>, delay: number) {
  const timers = useRef(new Map<string, number>());
  const pending = useRef(new Map<string, T>());
  const flush = useCallback(
    (key: string) => {
      const v = pending.current.get(key);
      pending.current.delete(key);
      timers.current.delete(key);
      if (v !== undefined) save(v).catch((e) => console.error("save failed", e));
    },
    [save],
  );
  const schedule = useCallback(
    (key: string, v: T) => {
      pending.current.set(key, v);
      window.clearTimeout(timers.current.get(key));
      timers.current.set(
        key,
        window.setTimeout(() => flush(key), delay),
      );
    },
    [delay, flush],
  );
  useEffect(() => {
    const flushAll = () => [...pending.current.keys()].forEach(flush);
    window.addEventListener("beforeunload", flushAll);
    return () => {
      window.removeEventListener("beforeunload", flushAll);
      flushAll();
    };
  }, [flush]);
  const hasPending = useCallback(() => pending.current.size > 0, []);
  // Stable object: effects that depend on the saver must not re-run on every render.
  return useMemo(() => ({ schedule, hasPending }), [schedule, hasPending]);
}

function Editor({ config, setConfig }: { config: Config; setConfig: (c: Config) => void }) {
  const t = useT();
  const [tab, setTab] = useState<Tab>(initialTab);
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [activeId, setActiveId] = useState("default");
  const [liveId, setLiveId] = useState<string | null>(null);
  const [path, setPath] = useState<PagePath>([]);
  const [selected, setSelected] = useState<number | null>(null);

  const profileSaver = useDebounced<Profile>(api.saveProfile, SAVE_DELAY);
  const configSaver = useDebounced<Config>(api.setConfig, SAVE_DELAY);

  const reload = useCallback(() => {
    api.listProfiles().then(setProfiles);
  }, []);

  useEffect(() => {
    reload();
    invoke<string>("active_profile")
      .then(setLiveId)
      .catch(() => {});
    const uns = [
      listen("profiles:changed", () => {
        if (!profileSaver.hasPending()) reload();
      }),
      listen<string>("profile:active", (e) => setLiveId(e.payload)),
      listen<Config>("config:changed", (e) => {
        // Ignore echoes of our own saves while a newer edit is still waiting to be saved.
        if (!configSaver.hasPending()) setConfig(e.payload);
      }),
      listen<string>("editor:select-tab", (e) => {
        const v = e.payload as Tab;
        if (v === "settings" || v === "plugins" || v === "profiles") setTab(v);
      }),
    ];
    return () => uns.forEach((u) => u.then((f) => f()));
  }, [reload, profileSaver, configSaver, setConfig]);

  useEffect(() => {
    document.title = APP_NAME;
  }, []);

  const profile = profiles.find((p) => p.id === activeId) ?? profiles[0] ?? null;
  const defaultProfile = profiles.find((p) => p.id === "default") ?? null;
  const page = profile ? getPage(profile, path) : null;
  const selectedButton = page && selected !== null && selected >= 0 ? page.slots[selected] : null;

  const updateProfile = useCallback(
    (p: Profile) => {
      setProfiles((list) => list.map((x) => (x.id === p.id ? p : x)));
      profileSaver.schedule(p.id, p);
    },
    [profileSaver],
  );

  const updateConfig = (c: Config) => {
    setConfig(c);
    configSaver.schedule("config", c);
  };

  const selectProfile = (id: string) => {
    setActiveId(id);
    setPath([]);
    setSelected(null);
  };

  const placeButton = (slot: number, b: Button) => {
    if (!profile || isReserved(profile.layout, path, slot)) return;
    const button =
      b.action.type === "folder"
        ? { ...b, action: { type: "folder" as const, page: emptyPage(profile.layout) } }
        : b;
    updateProfile(setSlot(profile, path, slot, button));
    setSelected(slot);
  };

  const plugin = usePluginPalette();

  const buttonFromPayload = (payload: DragPayload): Button | null => {
    if (payload.kind === "preset") {
      const preset = PRESETS.find((p) => p.id === payload.presetId);
      return preset ? buttonFromPreset(preset, t) : null;
    }
    if (payload.kind === "pluginAction")
      return plugin.makeButton(payload.pluginUuid, payload.actionUuid);
    return null;
  };

  const onDrop = (payload: DragPayload, slot: number) => {
    if (!profile) return;
    if (payload.kind === "slot") {
      if (isReserved(profile.layout, path, slot)) return;
      updateProfile(moveSlot(profile, path, payload.index, slot));
      setSelected(slot);
      return;
    }
    const b = buttonFromPayload(payload);
    if (b) placeButton(slot, b);
  };

  const onPaletteActivate = (item: PaletteItem) => {
    if (!profile || !page) return;
    // Click on an action: fill the selected empty slot, or the first free one.
    let slot = selected !== null && selected >= 0 && !page.slots[selected] ? selected : null;
    if (slot === null) {
      slot = page.slots.findIndex((s, i) => !s && !isReserved(profile.layout, path, i));
      if (slot < 0) return;
    }
    const b = buttonFromPayload(item.payload);
    if (b) placeButton(slot, b);
  };

  const createForApp = (app: AppInfo) => {
    const existing = profiles.find((p) => p.apps.includes(app.exe));
    if (existing) {
      selectProfile(existing.id);
      return;
    }
    const name = app.name.replace(/\.lnk$/i, "") || app.exe;
    const p = newProfile(name, [app.exe]);
    setProfiles((list) => [...list, p]);
    api.saveProfile(p).then(() => selectProfile(p.id));
  };

  const deleteProfile = async () => {
    if (!profile || profile.id === "default") return;
    if (!window.confirm(t("profile.confirmDelete", { name: profile.name }))) return;
    await api.deleteProfile(profile.id);
    setProfiles((l) => l.filter((p) => p.id !== profile.id));
    selectProfile("default");
  };

  const paletteItems = useMemo(() => [...presetItems(t), ...plugin.items], [t, plugin.items]);

  const tabs: { id: Tab; label: string; icon: React.ReactNode }[] = [
    { id: "profiles", label: t("tab.profiles"), icon: <LayoutGrid size={16} /> },
    { id: "settings", label: t("tab.settings"), icon: <Settings size={16} /> },
    { id: "plugins", label: t("tab.plugins"), icon: <Puzzle size={16} /> },
  ];

  return (
    <DndProvider onDrop={onDrop}>
      <div className="flex h-full flex-col bg-neutral-900 text-neutral-100">
        <header className="flex items-center gap-1 border-b border-neutral-800 px-3 py-2">
          <span className="mr-4 font-semibold tracking-tight">{APP_NAME}</span>
          {tabs.map((x) => (
            <button
              key={x.id}
              onClick={() => setTab(x.id)}
              className={cx(
                "flex items-center gap-1.5 rounded-md px-3 py-1.5 text-sm",
                tab === x.id ? "bg-neutral-800 text-white" : "text-neutral-400 hover:text-white",
              )}
            >
              {x.icon}
              {x.label}
            </button>
          ))}
          <div className="flex-1" />
          <span className="text-xs text-neutral-500">{t("editor.autosave")}</span>
        </header>

        {tab === "profiles" && profile && (
          <div className="flex min-h-0 flex-1">
            <aside className="w-60 shrink-0 border-r border-neutral-800">
              <ProfilesSidebar
                profiles={profiles}
                activeId={profile.id}
                liveId={liveId}
                onSelect={selectProfile}
                onCreateForApp={createForApp}
                onImported={(p) => {
                  setProfiles((l) => [...l.filter((x) => x.id !== p.id), p]);
                  selectProfile(p.id);
                }}
              />
            </aside>
            <main className="flex min-w-0 flex-1 flex-col">
              <ProfileSettings
                profile={profile}
                onChange={updateProfile}
                onDelete={deleteProfile}
              />
              <div className="min-h-0 flex-1">
                <PanelEditor
                  profile={profile}
                  defaultProfile={defaultProfile}
                  path={path}
                  onPath={(p) => {
                    setPath(p);
                    setSelected(null);
                  }}
                  selected={selected}
                  onSelect={setSelected}
                  appearance={config.appearance}
                  onDuplicate={(slot) => {
                    const r = duplicateSlot(profile, path, slot);
                    if (r) {
                      updateProfile(r.profile);
                      setSelected(r.index);
                    }
                  }}
                  onDelete={(slot) => {
                    updateProfile(setSlot(profile, path, slot, null));
                    setSelected(null);
                  }}
                  onMakeFolder={(slot) =>
                    updateProfile(makeFolder(profile, path, slot, t("act.folder")))
                  }
                />
              </div>
            </main>
            <aside className="w-80 shrink-0 border-l border-neutral-800">
              {selectedButton ? (
                <ButtonInspector
                  key={selectedButton.id}
                  button={selectedButton}
                  ctx={{ profiles }}
                  onChange={(b) => updateProfile(setSlot(profile, path, selected!, b))}
                  onDelete={() => {
                    updateProfile(setSlot(profile, path, selected!, null));
                    setSelected(null);
                  }}
                  onClose={() => setSelected(null)}
                  onOpenFolder={() => {
                    setPath([...path, selected!]);
                    setSelected(null);
                  }}
                  extra={
                    selectedButton.action.type === "plugin" ? (
                      <PluginInspector button={selectedButton} profile={profile} />
                    ) : undefined
                  }
                />
              ) : (
                <ActionPalette items={paletteItems} onActivate={onPaletteActivate} />
              )}
            </aside>
          </div>
        )}
        {tab === "settings" && <SettingsTab config={config} onChange={updateConfig} />}
        {tab === "plugins" && <PluginsTab />}
        {!config.firstRunDone && (
          <WelcomeWizard
            config={config}
            onDone={(created) => {
              setConfig({ ...config, firstRunDone: true });
              if (created.length) reload();
            }}
            onOpenPlugins={() => setTab("plugins")}
          />
        )}
      </div>
    </DndProvider>
  );
}
