// Small shared UI building blocks for the editor.
import { X } from "lucide-react";
import { useEffect, type ReactNode } from "react";

export function cx(...c: (string | false | null | undefined)[]): string {
  return c.filter(Boolean).join(" ");
}

export function Field({
  label,
  hint,
  error,
  children,
}: {
  label: string;
  hint?: string;
  error?: string | null;
  children: ReactNode;
}) {
  return (
    <label className="block space-y-1">
      <span className="block text-xs font-medium text-neutral-400">{label}</span>
      {children}
      {hint && !error && <span className="block text-xs text-neutral-500">{hint}</span>}
      {error && <span className="block text-xs text-rose-400">{error}</span>}
    </label>
  );
}

const inputCls =
  "w-full rounded-md border border-neutral-700 bg-neutral-800 px-2.5 py-1.5 text-sm text-neutral-100 " +
  "placeholder:text-neutral-500 focus:border-sky-500 focus:outline-none";

export function TextInput(
  props: Omit<React.InputHTMLAttributes<HTMLInputElement>, "onChange" | "value"> & {
    value: string;
    onChange: (v: string) => void;
  },
) {
  const { value, onChange, className, ...rest } = props;
  return (
    <input
      {...rest}
      className={cx(inputCls, className)}
      value={value}
      onChange={(e) => onChange(e.target.value)}
    />
  );
}

export function TextArea(props: {
  value: string;
  onChange: (v: string) => void;
  rows?: number;
  placeholder?: string;
}) {
  return (
    <textarea
      className={cx(inputCls, "resize-y font-mono")}
      rows={props.rows ?? 3}
      value={props.value}
      placeholder={props.placeholder}
      onChange={(e) => props.onChange(e.target.value)}
    />
  );
}

export function NumberInput(props: {
  value: number;
  onChange: (v: number) => void;
  min?: number;
  max?: number;
  step?: number;
}) {
  return (
    <input
      type="number"
      className={inputCls}
      value={Number.isFinite(props.value) ? props.value : ""}
      min={props.min}
      max={props.max}
      step={props.step}
      onChange={(e) => props.onChange(e.target.value === "" ? NaN : Number(e.target.value))}
    />
  );
}

export function Select<T extends string>(props: {
  value: T;
  onChange: (v: T) => void;
  options: { value: T; label: string }[];
}) {
  return (
    <select
      className={inputCls}
      value={props.value}
      onChange={(e) => props.onChange(e.target.value as T)}
    >
      {props.options.map((o) => (
        <option key={o.value} value={o.value}>
          {o.label}
        </option>
      ))}
    </select>
  );
}

export function Toggle(props: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label: string;
  hint?: string;
}) {
  return (
    <label className="flex cursor-pointer items-start gap-3 select-none">
      <button
        type="button"
        role="switch"
        aria-checked={props.checked}
        onClick={() => props.onChange(!props.checked)}
        className={cx(
          "relative mt-0.5 h-5 w-9 shrink-0 rounded-full transition-colors",
          props.checked ? "bg-sky-500" : "bg-neutral-600",
        )}
      >
        <span
          className={cx(
            "absolute top-0.5 h-4 w-4 rounded-full bg-white transition-all",
            props.checked ? "left-4.5" : "left-0.5",
          )}
        />
      </button>
      <span>
        <span className="block text-sm text-neutral-200">{props.label}</span>
        {props.hint && <span className="block text-xs text-neutral-500">{props.hint}</span>}
      </span>
    </label>
  );
}

export function Btn(
  props: React.ButtonHTMLAttributes<HTMLButtonElement> & {
    variant?: "primary" | "ghost" | "danger" | "default";
    small?: boolean;
  },
) {
  const { variant = "default", small, className, ...rest } = props;
  return (
    <button
      type="button"
      {...rest}
      className={cx(
        "inline-flex items-center justify-center gap-1.5 rounded-md font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-40",
        small ? "px-2 py-1 text-xs" : "px-3 py-1.5 text-sm",
        variant === "primary" && "bg-sky-600 text-white hover:bg-sky-500",
        variant === "default" && "bg-neutral-700 text-neutral-100 hover:bg-neutral-600",
        variant === "ghost" && "text-neutral-300 hover:bg-neutral-800 hover:text-white",
        variant === "danger" && "bg-rose-700/80 text-white hover:bg-rose-600",
        className,
      )}
    />
  );
}

export function Modal(props: {
  title: string;
  onClose: () => void;
  children: ReactNode;
  wide?: boolean;
  footer?: ReactNode;
}) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && props.onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [props]);
  return (
    <div
      className="fixed inset-0 z-40 flex items-center justify-center bg-black/60 p-6"
      onPointerDown={(e) => e.target === e.currentTarget && props.onClose()}
    >
      <div
        className={cx(
          "flex max-h-full w-full flex-col rounded-xl border border-neutral-700 bg-neutral-900 shadow-2xl",
          props.wide ? "max-w-3xl" : "max-w-lg",
        )}
      >
        <div className="flex items-center justify-between border-b border-neutral-800 px-4 py-3">
          <h2 className="text-base font-semibold text-neutral-100">{props.title}</h2>
          <button className="text-neutral-400 hover:text-white" onClick={props.onClose}>
            <X size={18} />
          </button>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto p-4">{props.children}</div>
        {props.footer && (
          <div className="flex justify-end gap-2 border-t border-neutral-800 px-4 py-3">
            {props.footer}
          </div>
        )}
      </div>
    </div>
  );
}

export function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="space-y-3">
      <h3 className="text-xs font-semibold tracking-wide text-neutral-500 uppercase">{title}</h3>
      {children}
    </section>
  );
}
