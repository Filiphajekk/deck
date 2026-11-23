// Centre: profile settings, folder breadcrumbs and the live panel preview with drag & drop
// and a context menu.
import { ChevronRight, Copy, Folder, FolderPlus, Trash2 } from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { PanelView } from "../components/PanelView";
import { useT } from "../i18n";
import { api } from "../lib/api";
import type { Appearance } from "../types/config";
import type { ButtonView, PanelGeometry } from "../types/overlay";
import type { Button, Profile } from "../types/profile";
import { useDnd } from "./dnd";
import { getPage, isReserved, type PagePath } from "./model";

function toView(b: Button | null): ButtonView | null {
  if (!b) return null;
  return {
    id: b.id,
    title: b.title,
    icon: b.icon,
    state: 0,
    isFolder: b.action.type === "folder",
    actionType: b.action.type,
  };
}

interface MenuState {
  x: number;
  y: number;
  slot: number;
}

export function PanelEditor({
  profile,
  defaultProfile,
  path,
  onPath,
  selected,
  onSelect,
  appearance,
  onDuplicate,
  onDelete,
  onMakeFolder,
}: {
  profile: Profile;
  defaultProfile: Profile | null;
  path: PagePath;
  onPath: (p: PagePath) => void;
  selected: number | null;
  onSelect: (slot: number | null) => void;
  appearance: Appearance;
  onDuplicate: (slot: number) => void;
  onDelete: (slot: number) => void;
  onMakeFolder: (slot: number) => void;
}) {
  const t = useT();
  const { begin, over, dragging } = useDnd();
  const [geometry, setGeometry] = useState<PanelGeometry | null>(null);
  const [menu, setMenu] = useState<MenuState | null>(null);
  const boxRef = useRef<HTMLDivElement>(null);
  const [fit, setFit] = useState(1);

  useEffect(() => {
    api.panelGeometry(profile.layout).then(setGeometry);
  }, [profile.layout, appearance.size]);

  // Scale the preview down if it does not fit the centre column.
  useLayoutEffect(() => {
    const el = boxRef.current;
    if (!el || !geometry) return;
    const update = () => {
      const r = el.getBoundingClientRect();
      setFit(Math.min(1, (r.width - 16) / geometry.width, (r.height - 16) / geometry.height));
    };
    update();
    const ro = new ResizeObserver(update);
    ro.observe(el);
    return () => ro.disconnect();
  }, [geometry]);

  useEffect(() => {
    if (!menu) return;
    const close = () => setMenu(null);
    window.addEventListener("pointerdown", close);
    window.addEventListener("blur", close);
    return () => {
      window.removeEventListener("pointerdown", close);
      window.removeEventListener("blur", close);
    };
  }, [menu]);

  const page = getPage(profile, path);
  if (!page) return null;

  // Empty top-level slots show the default profile's buttons faded ("fill from default").
  const ghosts =
    path.length === 0 &&
    profile.fillFromDefault &&
    defaultProfile &&
    defaultProfile.id !== profile.id
      ? defaultProfile.page.slots
      : null;

  const buttons = page.slots.map(toView);
  const crumbs: { title: string; path: PagePath }[] = [{ title: profile.name, path: [] }];
  {
    let p = profile.page;
    const acc: number[] = [];
    for (const i of path) {
      const b = p.slots[i];
      if (!b || b.action.type !== "folder") break;
      acc.push(i);
      crumbs.push({ title: b.title || t("act.folder"), path: [...acc] });
      p = b.action.page;
    }
  }

  const slotFromEvent = (e: React.PointerEvent | React.MouseEvent): number | null => {
    const el = (e.target as Element).closest("[data-slot]");
    if (!el) return null;
    const n = Number(el.getAttribute("data-slot"));
    return Number.isFinite(n) ? n : null;
  };

  const onPointerDown = (e: React.PointerEvent) => {
    const slot = slotFromEvent(e);
    if (slot === null) {
      if (e.button === 0) onSelect(null);
      return;
    }
    if (slot < 0) {
      // Centre: go one level up.
      if (e.button === 0 && path.length > 0) onPath(path.slice(0, -1));
      return;
    }
    if (isReserved(profile.layout, path, slot)) {
      if (e.button === 0) onPath(path.slice(0, -1));
      return;
    }
    const b = page.slots[slot];
    if (b && e.button === 0) {
      begin(
        e,
        { kind: "slot", index: slot },
        <span>{b.title || t(`type.${b.action.type}`)}</span>,
        () => onSelect(slot),
      );
    } else if (e.button === 0) {
      onSelect(slot);
    }
  };

  const onDoubleClick = (e: React.MouseEvent) => {
    const slot = slotFromEvent(e);
    if (slot === null || slot < 0) return;
    const b = page.slots[slot];
    if (b?.action.type === "folder") onPath([...path, slot]);
  };

  const onContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    const slot = slotFromEvent(e);
    if (slot === null || slot < 0 || isReserved(profile.layout, path, slot)) return;
    onSelect(slot);
    setMenu({ x: e.clientX, y: e.clientY, slot });
  };

  const menuButton = menu ? page.slots[menu.slot] : null;

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center gap-1 px-4 pt-3 text-sm text-neutral-400">
        {crumbs.map((c, i) => (
          <span key={i} className="flex items-center gap-1">
            {i > 0 && <ChevronRight size={14} className="text-neutral-600" />}
            <button
              className={i === crumbs.length - 1 ? "text-neutral-100" : "hover:text-white"}
              onClick={() => onPath(c.path)}
            >
              {i > 0 && <Folder size={13} className="mr-1 inline" />}
              {c.title}
            </button>
          </span>
        ))}
      </div>
      <div ref={boxRef} className="relative flex min-h-0 flex-1 items-center justify-center">
        {geometry && (
          <div
            data-drop-panel
            style={{ transform: `scale(${fit})` }}
            onPointerDown={onPointerDown}
            onDoubleClick={onDoubleClick}
            onContextMenu={onContextMenu}
            className="relative"
          >
            <PanelView
              layout={profile.layout}
              geometry={geometry}
              buttons={buttons}
              inFolder={path.length > 0}
              selected={dragging ? null : selected}
              dropTarget={dragging ? over : null}
              appearance={appearance}
              editor
              backLabel={t("overlay.back")}
              closeLabel={t("overlay.close")}
            />
            {ghosts && (
              <div className="pointer-events-none absolute inset-0 opacity-35">
                <PanelView
                  layout={profile.layout}
                  geometry={geometry}
                  buttons={ghosts.map((g, i) => (page.slots[i] ? null : toView(g)))}
                  inFolder={false}
                  selected={null}
                  appearance={{ ...appearance, opacity: 0 }}
                />
              </div>
            )}
          </div>
        )}
      </div>
      <p className="px-4 pb-3 text-center text-xs text-neutral-500">{t("editor.previewHint")}</p>

      {menu && (
        <div
          className="fixed z-50 min-w-44 rounded-lg border border-neutral-700 bg-neutral-800 py-1 text-sm shadow-xl"
          style={{ left: menu.x, top: menu.y }}
          onPointerDown={(e) => e.stopPropagation()}
        >
          {menuButton && (
            <MenuItem
              icon={<Copy size={14} />}
              label={t("menu.duplicate")}
              onClick={() => {
                onDuplicate(menu.slot);
                setMenu(null);
              }}
            />
          )}
          {menuButton?.action.type !== "folder" && (
            <MenuItem
              icon={<FolderPlus size={14} />}
              label={menuButton ? t("menu.moveToFolder") : t("menu.newFolder")}
              onClick={() => {
                onMakeFolder(menu.slot);
                setMenu(null);
              }}
            />
          )}
          {menuButton && (
            <MenuItem
              icon={<Trash2 size={14} />}
              label={t("common.delete")}
              danger
              onClick={() => {
                onDelete(menu.slot);
                setMenu(null);
              }}
            />
          )}
        </div>
      )}
    </div>
  );
}

function MenuItem(props: {
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
  danger?: boolean;
}) {
  return (
    <button
      className={
        "flex w-full items-center gap-2 px-3 py-1.5 text-left hover:bg-neutral-700 " +
        (props.danger ? "text-rose-400" : "text-neutral-200")
      }
      onClick={props.onClick}
    >
      {props.icon}
      {props.label}
    </button>
  );
}
