// Right column when nothing is selected: actions by category with search. Drag an action
// onto a slot, or click it to put it into the selected empty slot.
import { ChevronDown, ChevronRight } from "lucide-react";
import { useMemo, useState, type ReactNode } from "react";
import { useT } from "../i18n";
import { ICONS } from "../lib/icons";
import { CATEGORIES, PRESETS, type Category, type Preset } from "./catalog";
import { useDnd } from "./dnd";
import { TextInput } from "./ui";

export interface PaletteItem {
  key: string;
  category: Category;
  label: string;
  icon: ReactNode;
  payload: Parameters<ReturnType<typeof useDnd>["begin"]>[1];
  disabled?: boolean;
  note?: string;
}

export function presetItems(t: (k: string) => string): PaletteItem[] {
  return PRESETS.map((p: Preset) => {
    const C = ICONS[p.icon];
    return {
      key: p.id,
      category: p.category,
      label: t(p.label),
      icon: C ? <C size={16} /> : null,
      payload: { kind: "preset", presetId: p.id },
    };
  });
}

export function ActionPalette({
  items,
  onActivate,
  emptyHint,
}: {
  items: PaletteItem[];
  onActivate: (item: PaletteItem) => void;
  emptyHint?: ReactNode;
}) {
  const t = useT();
  const { begin } = useDnd();
  const [query, setQuery] = useState("");
  const [collapsed, setCollapsed] = useState<Set<Category>>(new Set());

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return q ? items.filter((i) => i.label.toLowerCase().includes(q)) : items;
  }, [items, query]);

  return (
    <div className="flex h-full flex-col">
      <div className="border-b border-neutral-800 p-3">
        <TextInput value={query} onChange={setQuery} placeholder={t("palette.search")} />
        <p className="mt-2 text-xs text-neutral-500">{t("palette.hint")}</p>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto p-2">
        {CATEGORIES.map((cat) => {
          const list = filtered.filter((i) => i.category === cat);
          if (list.length === 0 && (query || cat !== "plugins")) return null;
          const isCollapsed = collapsed.has(cat) && !query;
          return (
            <div key={cat} className="mb-2">
              <button
                className="flex w-full items-center gap-1 px-1 py-1 text-xs font-semibold tracking-wide text-neutral-500 uppercase hover:text-neutral-300"
                onClick={() =>
                  setCollapsed((s) => {
                    const n = new Set(s);
                    if (n.has(cat)) n.delete(cat);
                    else n.add(cat);
                    return n;
                  })
                }
              >
                {isCollapsed ? <ChevronRight size={14} /> : <ChevronDown size={14} />}
                {t(`cat.${cat}`)}
              </button>
              {!isCollapsed && (
                <div className="space-y-0.5">
                  {list.length === 0 && cat === "plugins" && (
                    <p className="px-2 py-1 text-xs text-neutral-600">
                      {emptyHint ?? t("palette.noPlugins")}
                    </p>
                  )}
                  {list.map((item) => (
                    <div
                      key={item.key}
                      onPointerDown={(e) =>
                        !item.disabled &&
                        begin(
                          e,
                          item.payload,
                          <span className="flex items-center gap-2">
                            {item.icon} {item.label}
                          </span>,
                          () => onActivate(item),
                        )
                      }
                      title={item.note}
                      className={
                        "flex cursor-grab items-center gap-2 rounded-md px-2 py-1.5 text-sm select-none " +
                        (item.disabled
                          ? "cursor-not-allowed text-neutral-600"
                          : "text-neutral-200 hover:bg-neutral-800 active:cursor-grabbing")
                      }
                    >
                      <span className="text-neutral-400">{item.icon}</span>
                      <span className="flex-1 truncate">{item.label}</span>
                      {item.note && <span className="text-[10px] text-amber-500">{item.note}</span>}
                    </div>
                  ))}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
