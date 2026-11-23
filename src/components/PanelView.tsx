// The panel as drawn in the overlay and in the editor preview (same look in both).
// All geometry comes from the backend (PanelGeometry) so drawing matches hit testing.
import { ArrowLeft, Folder, Plus, X } from "lucide-react";
import type { CSSProperties, ReactNode } from "react";
import { ICONS } from "../lib/icons";
import type { Appearance } from "../types/config";
import type { ButtonView, PanelGeometry } from "../types/overlay";
import type { Icon, Layout } from "../types/profile";

export interface PanelViewProps {
  layout: Layout;
  geometry: PanelGeometry;
  buttons: (ButtonView | null)[];
  inFolder: boolean;
  /** Selected slot, -1 = centre. */
  selected: number | null;
  appearance: Appearance;
  /** Button id showing a red "!". */
  alert?: string | null;
  /** Editor: show "+" in empty slots and make slots look clickable. */
  editor?: boolean;
  /** Editor: slot being dragged over (drop target highlight). */
  dropTarget?: number | null;
  backLabel?: string;
  closeLabel?: string;
}

export function hexToRgba(hex: string, alpha: number): string {
  const m = /^#([0-9a-f]{6})$/i.exec(hex);
  const n = m ? parseInt(m[1], 16) : 0x38bdf8;
  return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, ${alpha})`;
}

const rad = (deg: number) => (deg * Math.PI) / 180;

/** Point at `r` from the centre, angle in degrees clockwise from "up". */
export function polar(c: number, r: number, deg: number): [number, number] {
  return [c + r * Math.sin(rad(deg)), c - r * Math.cos(rad(deg))];
}

export function wedgePath(c: number, rIn: number, rOut: number, a0: number, a1: number): string {
  const [x0, y0] = polar(c, rOut, a0);
  const [x1, y1] = polar(c, rOut, a1);
  const [x2, y2] = polar(c, rIn, a1);
  const [x3, y3] = polar(c, rIn, a0);
  const f = (n: number) => n.toFixed(2);
  return (
    `M${f(x0)} ${f(y0)} A${f(rOut)} ${f(rOut)} 0 0 1 ${f(x1)} ${f(y1)} ` +
    `L${f(x2)} ${f(y2)} A${f(rIn)} ${f(rIn)} 0 0 0 ${f(x3)} ${f(y3)} Z`
  );
}

export function IconView({
  icon,
  size,
  className,
}: {
  icon?: Icon | null;
  size: number;
  className?: string;
}) {
  if (!icon) return null;
  if (icon.kind === "lucide") {
    const C = ICONS[icon.name];
    return C ? <C size={size} strokeWidth={1.8} className={className} /> : null;
  }
  if (icon.kind === "emoji") {
    return (
      <span style={{ fontSize: size * 0.95, lineHeight: 1 }} className={className}>
        {icon.value}
      </span>
    );
  }
  return (
    <img
      src={icon.src}
      alt=""
      draggable={false}
      style={{ width: size * 1.25, height: size * 1.25, objectFit: "contain" }}
      className={className}
    />
  );
}

function SlotContent({
  view,
  size,
  alert,
  editor,
}: {
  view: ButtonView | null;
  size: number;
  alert: boolean;
  editor?: boolean;
}) {
  if (!view) {
    return editor ? <Plus size={size * 0.6} className="text-white/20" /> : null;
  }
  const iconSize = size;
  const on = view.state === 1;
  return (
    <div className="relative flex flex-col items-center justify-center gap-1 text-center">
      <div className={on ? "text-rose-300" : "text-white/90"}>
        {view.icon ? (
          <IconView icon={view.icon} size={iconSize} />
        ) : (
          <span className="block" style={{ height: iconSize }} />
        )}
      </div>
      {view.title && (
        <div
          className="line-clamp-2 max-w-full leading-tight font-medium break-words text-white/85"
          style={{ fontSize: Math.max(10, size * 0.42) }}
        >
          {view.title}
        </div>
      )}
      {view.isFolder && (
        <Folder
          size={size * 0.42}
          className="absolute -top-1 -right-3 text-white/50"
          fill="currentColor"
        />
      )}
      {alert && (
        <span
          className="absolute -top-2 -right-3 flex items-center justify-center rounded-full bg-red-500 font-bold text-white shadow"
          style={{ width: size * 0.62, height: size * 0.62, fontSize: size * 0.45 }}
        >
          !
        </span>
      )}
    </div>
  );
}

export function PanelView(props: PanelViewProps) {
  const { geometry: g, appearance } = props;
  const bg = `rgba(17, 20, 28, ${appearance.opacity})`;
  const style: CSSProperties = { width: g.width, height: g.height, position: "relative" };
  return (
    <div style={style}>
      {props.layout.kind === "radial" ? (
        <RadialPanel {...props} bg={bg} />
      ) : (
        <GridPanel {...props} bg={bg} />
      )}
    </div>
  );
}

function RadialPanel(props: PanelViewProps & { bg: string }) {
  const { geometry: g, appearance, selected, buttons, dropTarget } = props;
  const c = g.width / 2;
  const rIn = g.inner + 3;
  const rOut = g.outer - 3;
  const rMid = (rIn + rOut) / 2 + 2;
  const accent = appearance.accent;
  const iconSize = Math.round(g.outer * 0.15);
  const labelW = (rOut - rIn) * 0.95;
  const slots: ReactNode[] = [];
  const labels: ReactNode[] = [];
  for (let i = 0; i < 8; i++) {
    const a = i * 45;
    const sel = selected === i;
    const drop = dropTarget === i;
    slots.push(
      <path
        key={i}
        data-slot={i}
        d={wedgePath(c, rIn, rOut, a - 22.5 + 0.9, a + 22.5 - 0.9)}
        fill={
          sel ? hexToRgba(accent, 0.8) : drop ? hexToRgba(accent, 0.45) : "rgba(255,255,255,0.045)"
        }
        stroke={drop ? accent : "rgba(255,255,255,0.06)"}
        strokeWidth={1}
        style={{ transition: appearance.animations ? "fill 80ms" : undefined }}
        className={props.editor ? "cursor-pointer hover:brightness-150" : undefined}
      />,
    );
    const [x, y] = polar(c, rMid, a);
    labels.push(
      <div
        key={i}
        className="pointer-events-none absolute flex items-center justify-center"
        style={{ left: x - labelW / 2, top: y - labelW / 2, width: labelW, height: labelW }}
      >
        <SlotContent
          view={buttons[i] ?? null}
          size={iconSize}
          alert={!!buttons[i] && buttons[i]!.id === props.alert}
          editor={props.editor}
        />
      </div>,
    );
  }
  const centerSel = selected === -1;
  const CenterIcon = props.inFolder ? ArrowLeft : X;
  return (
    <>
      <svg width={g.width} height={g.height} className="absolute inset-0 overflow-visible">
        <circle
          cx={c}
          cy={c}
          r={g.outer}
          fill={props.bg}
          stroke="rgba(255,255,255,0.08)"
          style={{ filter: "drop-shadow(0 6px 16px rgba(0,0,0,0.45))" }}
        />
        {slots}
        <circle
          data-slot={-1}
          cx={c}
          cy={c}
          r={g.inner - 1}
          fill={centerSel ? hexToRgba(accent, 0.8) : "rgba(255,255,255,0.07)"}
          stroke="rgba(255,255,255,0.08)"
          className={props.editor ? "cursor-pointer" : undefined}
        />
      </svg>
      {labels}
      <div
        className="pointer-events-none absolute flex flex-col items-center justify-center text-white/70"
        style={{ left: c - g.inner, top: c - g.inner, width: g.inner * 2, height: g.inner * 2 }}
        title={props.inFolder ? props.backLabel : props.closeLabel}
      >
        <CenterIcon size={iconSize} />
      </div>
    </>
  );
}

function GridPanel(props: PanelViewProps & { bg: string }) {
  const { geometry: g, appearance, selected, buttons, dropTarget } = props;
  const accent = appearance.accent;
  const iconSize = Math.round(g.cell * 0.3);
  const cells: ReactNode[] = [];
  for (let i = 0; i < g.cols * g.rows; i++) {
    const col = i % g.cols;
    const row = Math.floor(i / g.cols);
    const left = g.margin + g.pad + col * (g.cell + g.gap);
    const top = g.margin + g.pad + row * (g.cell + g.gap);
    const sel = selected === i;
    const drop = dropTarget === i;
    const isBack = props.inFolder && i === 0;
    cells.push(
      <div
        key={i}
        data-slot={i}
        className={
          "absolute flex items-center justify-center overflow-hidden rounded-xl p-1 " +
          (props.editor && !isBack ? "cursor-pointer hover:brightness-150" : "")
        }
        style={{
          left,
          top,
          width: g.cell,
          height: g.cell,
          background: sel
            ? hexToRgba(accent, 0.8)
            : drop
              ? hexToRgba(accent, 0.45)
              : "rgba(255,255,255,0.05)",
          border: `1px solid ${drop ? accent : "rgba(255,255,255,0.07)"}`,
          transition: appearance.animations ? "background 80ms" : undefined,
        }}
      >
        <div className="pointer-events-none">
          {isBack ? (
            <div className="flex flex-col items-center gap-1 text-white/70">
              <ArrowLeft size={iconSize} />
              <span style={{ fontSize: Math.max(10, iconSize * 0.42) }}>{props.backLabel}</span>
            </div>
          ) : (
            <SlotContent
              view={buttons[i] ?? null}
              size={iconSize}
              alert={!!buttons[i] && buttons[i]!.id === props.alert}
              editor={props.editor}
            />
          )}
        </div>
      </div>,
    );
  }
  return (
    <>
      <div
        className="absolute rounded-2xl"
        style={{
          left: g.margin,
          top: g.margin,
          width: g.width - g.margin * 2,
          height: g.height - g.margin * 2,
          background: props.bg,
          border: "1px solid rgba(255,255,255,0.08)",
          boxShadow: "0 6px 16px rgba(0,0,0,0.45)",
        }}
      />
      {cells}
    </>
  );
}
