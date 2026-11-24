// Plugin store: a curated catalog of open-source Stream Deck plugins that work on Windows.
// Plugins are downloaded from their authors' GitHub releases when the user installs them.
import { invoke } from "@tauri-apps/api/core";
import {
  AppWindow,
  Check,
  Code,
  Cpu,
  ExternalLink,
  Gamepad2,
  Globe,
  House,
  LoaderCircle,
  MessageCircle,
  Music,
  Puzzle,
  RadioTower,
  RotateCw,
  SlidersHorizontal,
  Timer,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { useLang, useT } from "../i18n";
import { errorText } from "./errorText";
import { Btn, cx, TextInput } from "./ui";

export interface StoreItem {
  id: string;
  uuid: string;
  name: string;
  author: string;
  repo: string;
  version: string;
  category: string;
  license: string;
  runtime: "exe" | "node" | "html";
  description: { cs: string; en: string };
  requires?: { cs: string; en: string } | null;
  installed: boolean;
  installedVersion: string | null;
}

export const STORE_CATEGORIES: { id: string; icon: ReactNode }[] = [
  { id: "communication", icon: <MessageCircle size={15} /> },
  { id: "music", icon: <Music size={15} /> },
  { id: "audio", icon: <SlidersHorizontal size={15} /> },
  { id: "streaming", icon: <RadioTower size={15} /> },
  { id: "smarthome", icon: <House size={15} /> },
  { id: "system", icon: <Cpu size={15} /> },
  { id: "productivity", icon: <Timer size={15} /> },
  { id: "info", icon: <Globe size={15} /> },
  { id: "developer", icon: <Code size={15} /> },
  { id: "games", icon: <Gamepad2 size={15} /> },
];

/** Filters the catalog by category and a search text (name, author, description). */
export function filterStore(
  items: StoreItem[],
  category: string | null,
  query: string,
  lang: "cs" | "en",
): StoreItem[] {
  const q = query.trim().toLowerCase();
  return items.filter(
    (i) =>
      (!category || i.category === category) &&
      (!q ||
        i.name.toLowerCase().includes(q) ||
        i.author.toLowerCase().includes(q) ||
        i.description[lang].toLowerCase().includes(q)),
  );
}

export function StoreView({ nodeAvailable }: { nodeAvailable: boolean }) {
  const t = useT();
  const lang = useLang();
  const [items, setItems] = useState<StoreItem[]>([]);
  const [category, setCategory] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState<Set<string>>(new Set());
  const [messages, setMessages] = useState<Record<string, { ok: boolean; text: string }>>({});

  const reload = useCallback(() => {
    invoke<StoreItem[]>("store_catalog")
      .then(setItems)
      .catch(() => setItems([]));
  }, []);
  useEffect(reload, [reload]);

  const shown = useMemo(
    () => filterStore(items, category, query, lang),
    [items, category, query, lang],
  );

  const install = async (item: StoreItem) => {
    setBusy((s) => new Set(s).add(item.id));
    setMessages((m) => ({ ...m, [item.id]: { ok: true, text: t("store.downloading") } }));
    try {
      await invoke("store_install", { id: item.id });
      setMessages((m) => ({ ...m, [item.id]: { ok: true, text: t("store.installedNow") } }));
    } catch (e) {
      setMessages((m) => ({ ...m, [item.id]: { ok: false, text: errorText(t, String(e)) } }));
    } finally {
      setBusy((s) => {
        const n = new Set(s);
        n.delete(item.id);
        return n;
      });
      reload();
    }
  };

  return (
    <div className="space-y-4">
      <p className="text-sm text-neutral-400">{t("store.intro")}</p>
      <div className="flex flex-wrap items-center gap-2">
        <div className="w-64">
          <TextInput value={query} onChange={setQuery} placeholder={t("store.search")} />
        </div>
        <CategoryChip active={category === null} onClick={() => setCategory(null)}>
          {t("store.all")} ({items.length})
        </CategoryChip>
        {STORE_CATEGORIES.filter((c) => items.some((i) => i.category === c.id)).map((c) => (
          <CategoryChip key={c.id} active={category === c.id} onClick={() => setCategory(c.id)}>
            {c.icon}
            {t(`store.cat.${c.id}`)}
          </CategoryChip>
        ))}
      </div>

      {shown.length === 0 && <p className="text-sm text-neutral-500">{t("store.nothing")}</p>}

      <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
        {shown.map((item) => {
          const cat = STORE_CATEGORIES.find((c) => c.id === item.category);
          const msg = messages[item.id];
          const needsNode = item.runtime === "node" && !nodeAvailable;
          return (
            <div
              key={item.id}
              className="flex flex-col rounded-xl border border-neutral-800 bg-neutral-900 p-4"
            >
              <div className="flex items-start gap-3">
                <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-neutral-800 text-sky-300">
                  {cat?.icon ?? <Puzzle size={15} />}
                </div>
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="font-medium text-neutral-100">{item.name}</span>
                    {item.installed && (
                      <span className="flex items-center gap-1 rounded-full bg-emerald-900/60 px-2 py-0.5 text-[11px] text-emerald-300">
                        <Check size={11} /> {t("store.installed")}
                      </span>
                    )}
                  </div>
                  <p className="truncate text-xs text-neutral-500">
                    {item.author} · {t(`store.cat.${item.category}`)} · {item.license}
                  </p>
                </div>
              </div>
              <p className="mt-2 flex-1 text-sm text-neutral-300">{item.description[lang]}</p>
              {item.requires && (
                <p className="mt-1 text-xs text-amber-400/90">{item.requires[lang]}</p>
              )}
              {needsNode && <p className="mt-1 text-xs text-amber-400">{t("store.needsNode")}</p>}
              {msg && (
                <p
                  className={cx(
                    "mt-1 text-xs break-words",
                    msg.ok ? "text-emerald-400" : "text-rose-400",
                  )}
                >
                  {msg.text}
                </p>
              )}
              <div className="mt-3 flex items-center gap-2">
                <Btn
                  small
                  variant={item.installed ? "default" : "primary"}
                  disabled={busy.has(item.id)}
                  onClick={() => install(item)}
                >
                  {busy.has(item.id) ? (
                    <LoaderCircle size={14} className="animate-spin" />
                  ) : item.installed ? (
                    <RotateCw size={14} />
                  ) : (
                    <AppWindow size={14} />
                  )}
                  {busy.has(item.id)
                    ? t("store.installing")
                    : item.installed
                      ? t("store.update")
                      : t("store.install")}
                </Btn>
                <Btn
                  small
                  variant="ghost"
                  onClick={() => invoke("open_url", { url: `https://github.com/${item.repo}` })}
                >
                  <ExternalLink size={14} /> GitHub
                </Btn>
                <span className="ml-auto text-xs text-neutral-600">
                  {item.installedVersion ?? item.version}
                </span>
              </div>
            </div>
          );
        })}
      </div>
      <p className="text-xs text-neutral-600">{t("store.disclaimer")}</p>
    </div>
  );
}

function CategoryChip(props: { active: boolean; onClick: () => void; children: ReactNode }) {
  return (
    <button
      onClick={props.onClick}
      className={cx(
        "flex items-center gap-1.5 rounded-full px-3 py-1 text-xs",
        props.active
          ? "bg-sky-600 text-white"
          : "bg-neutral-800 text-neutral-300 hover:bg-neutral-700",
      )}
    >
      {props.children}
    </button>
  );
}
