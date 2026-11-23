// Stream Deck plugins in the editor: palette items, Property Inspector and the Plugins tab.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { AlertTriangle, ExternalLink, Puzzle, RotateCw, Trash2, Upload } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useT } from "../i18n";
import { newId } from "../lib/ids";
import type { Button, Icon, Profile, Visual } from "../types/profile";
import type { PaletteItem } from "./ActionPalette";
import { Btn, cx } from "./ui";
import { MARKETPLACE_URL } from "./WelcomeWizard";

export type PluginStatus =
  | { kind: "starting" }
  | { kind: "running" }
  | { kind: "stopped" }
  | { kind: "error"; message: string }
  | { kind: "incompatible"; message: string }
  | { kind: "nodeMissing" };

export interface PluginActionInfo {
  uuid: string;
  name: string;
  tooltip: string;
  icon: string | null;
  supported: boolean;
  hasPropertyInspector: boolean;
}

export interface PluginInfo {
  uuid: string;
  name: string;
  version: string;
  author: string;
  description: string;
  icon: string | null;
  status: PluginStatus;
  actions: PluginActionInfo[];
}

interface Defaults {
  title: string;
  icon: Icon | null;
  stateB: Visual | null;
}

const NODE_URL = "https://nodejs.org/";

/** Backend errors of the form `i18n:<key>` are translated; anything else is shown as is. */
function errorText(t: (k: string) => string, msg: string): string {
  const m = /^(?:Error: )?i18n:([w.]+)/.exec(msg);
  return m ? t(m[1]) : msg;
}

function usePlugins() {
  const [plugins, setPlugins] = useState<PluginInfo[]>([]);
  const reload = useCallback(() => {
    invoke<PluginInfo[]>("list_plugins")
      .then(setPlugins)
      .catch(() => setPlugins([]));
  }, []);
  useEffect(() => {
    reload();
    const un = listen("plugins:changed", reload);
    return () => {
      un.then((f) => f());
    };
  }, [reload]);
  return { plugins, reload };
}

/** Palette entries for plugin actions and a factory for new plugin buttons. */
export function usePluginPalette(): {
  items: PaletteItem[];
  makeButton: (pluginUuid: string, actionUuid: string) => Button | null;
} {
  const t = useT();
  const { plugins } = usePlugins();
  const [defaults, setDefaults] = useState<Map<string, Defaults>>(new Map());

  useEffect(() => {
    let alive = true;
    const keys = plugins.flatMap((p) => p.actions.map((a) => [p.uuid, a.uuid] as const));
    Promise.all(
      keys.map(([p, a]) =>
        invoke<Defaults>("plugin_action_defaults", { pluginUuid: p, actionUuid: a })
          .then((d): [string, Defaults] => [`${p}/${a}`, d])
          .catch(() => null),
      ),
    ).then((list) => {
      if (!alive) return;
      setDefaults(new Map(list.filter((x): x is [string, Defaults] => !!x)));
    });
    return () => {
      alive = false;
    };
  }, [plugins]);

  const items: PaletteItem[] = plugins.flatMap((p) =>
    p.actions.map((a) => ({
      key: `${p.uuid}/${a.uuid}`,
      category: "plugins" as const,
      label: plugins.length > 1 ? `${a.name} (${p.name})` : a.name,
      icon: a.icon ? (
        <img src={a.icon} alt="" className="h-4 w-4 object-contain" />
      ) : (
        <Puzzle size={16} />
      ),
      payload: { kind: "pluginAction" as const, pluginUuid: p.uuid, actionUuid: a.uuid },
      disabled: !a.supported,
      note: a.supported ? undefined : t("plugins.encoderOnly"),
    })),
  );

  const makeButton = (pluginUuid: string, actionUuid: string): Button | null => {
    const plugin = plugins.find((p) => p.uuid === pluginUuid);
    const action = plugin?.actions.find((a) => a.uuid === actionUuid);
    if (!plugin || !action || !action.supported) return null;
    const d = defaults.get(`${pluginUuid}/${actionUuid}`);
    return {
      id: newId(),
      title: d?.title ?? action.name,
      icon: d?.icon ?? (action.icon ? { kind: "image", src: action.icon } : null),
      stateB: d?.stateB ?? null,
      action: { type: "plugin", pluginUuid, actionUuid },
    };
  };

  return { items, makeButton };
}

function statusLabel(t: (k: string) => string, s: PluginStatus): string {
  return t(`plugins.status.${s.kind}`);
}

/** Property Inspector of the selected plugin button, in an iframe (SPEC 7.5). */
export function PluginInspector({ button }: { button: Button; profile: Profile }) {
  const t = useT();
  const { plugins } = usePlugins();
  const [url, setUrl] = useState<string | null | undefined>(undefined);
  const action = button.action.type === "plugin" ? button.action : null;
  const plugin = plugins.find((p) => p.uuid === action?.pluginUuid);

  useEffect(() => {
    if (!action) return;
    setUrl(undefined);
    invoke<string | null>("property_inspector_url", {
      pluginUuid: action.pluginUuid,
      actionUuid: action.actionUuid,
      context: button.id,
    })
      .then(setUrl)
      .catch(() => setUrl(null));
    // Re-create the inspector when the plugin (re)starts so it reconnects.
  }, [action?.pluginUuid, action?.actionUuid, button.id, plugin?.status.kind]); // eslint-disable-line react-hooks/exhaustive-deps

  if (!action) return null;
  if (!plugin) {
    return <p className="text-sm text-amber-400">{t("plugins.missing")}</p>;
  }
  return (
    <div className="space-y-2">
      <p className="text-xs text-neutral-500">
        {plugin.name} · {statusLabel(t, plugin.status)}
      </p>
      {plugin.status.kind === "nodeMissing" && <NodeMissing />}
      {url === null && <p className="text-sm text-neutral-500">{t("plugins.noInspector")}</p>}
      {url && (
        <iframe
          key={url}
          src={url}
          title="Property Inspector"
          className="h-[420px] w-full rounded-md border border-neutral-800 bg-[#2d2d2d]"
        />
      )}
    </div>
  );
}

function NodeMissing() {
  const t = useT();
  return (
    <div className="flex items-start gap-2 rounded-md border border-amber-700/50 bg-amber-950/30 p-2 text-xs text-amber-300">
      <AlertTriangle size={14} className="mt-0.5 shrink-0" />
      <span>
        {t("plugins.nodeMissing")}{" "}
        <button className="underline" onClick={() => invoke("open_url", { url: NODE_URL })}>
          nodejs.org
        </button>
      </span>
    </div>
  );
}

export function PluginsTab() {
  const t = useT();
  const { plugins, reload } = usePlugins();
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    const un = listen<{ ok: boolean; name?: string; error?: string }>("plugins:installed", (e) => {
      setMessage(
        e.payload.ok
          ? { ok: true, text: t("plugins.installed", { name: e.payload.name ?? "" }) }
          : { ok: false, text: errorText(t, e.payload.error ?? "") },
      );
      reload();
    });
    return () => {
      un.then((f) => f());
    };
  }, [t, reload]);

  const installFromFile = async () => {
    const path = await open({
      multiple: false,
      filters: [{ name: "Stream Deck plugin", extensions: ["streamDeckPlugin"] }],
    });
    if (typeof path !== "string") return;
    setBusy(true);
    setMessage(null);
    try {
      const info = await invoke<PluginInfo>("install_plugin", { path });
      setMessage({ ok: true, text: t("plugins.installed", { name: info.name }) });
    } catch (e) {
      setMessage({ ok: false, text: errorText(t, String(e)) });
    } finally {
      setBusy(false);
      reload();
    }
  };

  const uninstall = async (p: PluginInfo) => {
    if (!window.confirm(t("plugins.confirmUninstall", { name: p.name }))) return;
    try {
      await invoke("uninstall_plugin", { uuid: p.uuid });
    } catch (e) {
      setMessage({ ok: false, text: errorText(t, String(e)) });
    }
    reload();
  };

  const anyNodeMissing = plugins.some((p) => p.status.kind === "nodeMissing");

  return (
    <div className="h-full overflow-y-auto">
      <div className="mx-auto max-w-3xl space-y-5 p-6">
        <div className="flex flex-wrap items-center gap-2">
          <Btn variant="primary" onClick={installFromFile} disabled={busy}>
            <Upload size={15} /> {t("plugins.install")}
          </Btn>
          <Btn variant="ghost" onClick={() => invoke("open_url", { url: MARKETPLACE_URL })}>
            <ExternalLink size={15} /> {t("plugins.marketplace")}
          </Btn>
        </div>
        <p className="text-sm text-neutral-400">{t("plugins.howTo")}</p>
        {message && (
          <p
            className={
              message.ok ? "text-sm text-emerald-400" : "text-sm break-words text-rose-400"
            }
          >
            {message.text}
          </p>
        )}
        {anyNodeMissing && <NodeMissing />}

        {plugins.length === 0 && (
          <div className="rounded-xl border border-dashed border-neutral-700 p-10 text-center text-sm text-neutral-500">
            <Puzzle className="mx-auto mb-2" />
            {t("plugins.empty")}
          </div>
        )}

        <div className="space-y-3">
          {plugins.map((p) => (
            <div key={p.uuid} className="rounded-xl border border-neutral-800 bg-neutral-900 p-4">
              <div className="flex items-start gap-3">
                {p.icon ? (
                  <img src={p.icon} alt="" className="h-10 w-10 rounded-md object-contain" />
                ) : (
                  <Puzzle className="h-10 w-10 text-neutral-500" />
                )}
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="font-medium text-neutral-100">{p.name}</span>
                    <span className="text-xs text-neutral-500">{p.version}</span>
                    <StatusBadge status={p.status} />
                  </div>
                  <p className="truncate text-xs text-neutral-500">
                    {p.author} · {p.uuid}
                  </p>
                  {p.description && (
                    <p className="mt-1 text-sm text-neutral-400">{p.description}</p>
                  )}
                  {(p.status.kind === "error" || p.status.kind === "incompatible") && (
                    <p className="mt-1 text-xs text-rose-400">{p.status.message}</p>
                  )}
                  <div className="mt-2 flex flex-wrap gap-1">
                    {p.actions.map((a) => (
                      <span
                        key={a.uuid}
                        title={a.supported ? a.tooltip : t("plugins.encoderOnly")}
                        className={cx(
                          "rounded-full px-2 py-0.5 text-xs",
                          a.supported
                            ? "bg-neutral-800 text-neutral-300"
                            : "bg-neutral-800/50 text-neutral-600 line-through",
                        )}
                      >
                        {a.name}
                      </span>
                    ))}
                  </div>
                </div>
                <div className="flex gap-1">
                  <Btn
                    small
                    variant="ghost"
                    title={t("plugins.restart")}
                    onClick={() => invoke("restart_plugin", { uuid: p.uuid }).then(reload)}
                  >
                    <RotateCw size={14} />
                  </Btn>
                  <Btn
                    small
                    variant="ghost"
                    className="text-rose-400"
                    title={t("plugins.uninstall")}
                    onClick={() => uninstall(p)}
                  >
                    <Trash2 size={14} />
                  </Btn>
                </div>
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

function StatusBadge({ status }: { status: PluginStatus }) {
  const t = useT();
  const color =
    status.kind === "running"
      ? "bg-emerald-900/60 text-emerald-300"
      : status.kind === "starting"
        ? "bg-sky-900/60 text-sky-300"
        : status.kind === "stopped"
          ? "bg-neutral-800 text-neutral-400"
          : "bg-rose-900/60 text-rose-300";
  return (
    <span className={cx("rounded-full px-2 py-0.5 text-[11px]", color)}>
      {statusLabel(t, status)}
    </span>
  );
}
