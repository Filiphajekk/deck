// Pointer-based drag & drop (HTML5 DnD does not work while Tauri's native file drop is on).
// Drop targets are elements with `data-slot` inside an element with `data-drop-panel`.
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import type { ActionType } from "../types/profile";

export type DragPayload =
  | { kind: "preset"; presetId: string }
  | { kind: "slot"; index: number }
  | { kind: "pluginAction"; pluginUuid: string; actionUuid: string; actionType?: ActionType };

interface DragState {
  payload: DragPayload;
  ghost: ReactNode;
  x: number;
  y: number;
}

interface DndApi {
  /** Call from onPointerDown; dragging starts after the pointer moved a few pixels. */
  begin: (
    e: React.PointerEvent,
    payload: DragPayload,
    ghost: ReactNode,
    onClick?: () => void,
  ) => void;
  dragging: DragPayload | null;
  /** Slot currently under the pointer while dragging (for highlight). */
  over: number | null;
}

const Ctx = createContext<DndApi>({ begin: () => {}, dragging: null, over: null });

export function useDnd() {
  return useContext(Ctx);
}

export function slotAt(x: number, y: number): number | null {
  const el = document.elementFromPoint(x, y);
  const slotEl = el?.closest("[data-slot]");
  if (!slotEl || !slotEl.closest("[data-drop-panel]")) return null;
  const n = Number(slotEl.getAttribute("data-slot"));
  return Number.isFinite(n) ? n : null;
}

export function DndProvider({
  children,
  onDrop,
}: {
  children: ReactNode;
  onDrop: (payload: DragPayload, slot: number) => void;
}) {
  const [drag, setDrag] = useState<DragState | null>(null);
  const [over, setOver] = useState<number | null>(null);
  const onDropRef = useRef(onDrop);
  useEffect(() => {
    onDropRef.current = onDrop;
  }, [onDrop]);

  const begin = useCallback<DndApi["begin"]>((e, payload, ghost, onClick) => {
    if (e.button !== 0) return;
    const sx = e.clientX;
    const sy = e.clientY;
    let started = false;
    const move = (ev: PointerEvent) => {
      if (!started && Math.hypot(ev.clientX - sx, ev.clientY - sy) < 5) return;
      started = true;
      setDrag({ payload, ghost, x: ev.clientX, y: ev.clientY });
      setOver(slotAt(ev.clientX, ev.clientY));
    };
    const up = (ev: PointerEvent) => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      if (started) {
        const slot = slotAt(ev.clientX, ev.clientY);
        setDrag(null);
        setOver(null);
        if (slot !== null && slot >= 0) onDropRef.current(payload, slot);
      } else {
        onClick?.();
      }
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }, []);

  return (
    <Ctx.Provider value={{ begin, dragging: drag?.payload ?? null, over }}>
      {children}
      {drag &&
        createPortal(
          <div
            className="pointer-events-none fixed z-50 -translate-x-1/2 -translate-y-1/2 rounded-lg border border-sky-400/60 bg-neutral-800/90 px-3 py-2 text-sm text-white shadow-xl"
            style={{ left: drag.x, top: drag.y }}
          >
            {drag.ghost}
          </div>,
          document.body,
        )}
    </Ctx.Provider>
  );
}
