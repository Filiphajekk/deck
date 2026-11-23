// Right column when a button is selected: title, icon, second state, action parameters, "Try it".
import { FolderOpen, Play, Trash2, X } from "lucide-react";
import { useState, type ReactNode } from "react";
import { IconView } from "../components/PanelView";
import { useT } from "../i18n";
import { api } from "../lib/api";
import type { Button, Icon, Visual } from "../types/profile";
import { ActionForm, type FormContext } from "./ActionForm";
import { hasStateB } from "./catalog";
import { IconPicker } from "./IconPicker";
import { validateButton } from "./validation";
import { Btn, Field, TextInput } from "./ui";

function IconButton({ icon, onPick }: { icon?: Icon | null; onPick: (i: Icon | null) => void }) {
  const t = useT();
  const [open, setOpen] = useState(false);
  return (
    <>
      <button
        onClick={() => setOpen(true)}
        title={t("icon.change")}
        className="flex h-14 w-14 shrink-0 items-center justify-center rounded-lg border border-neutral-700 bg-neutral-800 text-neutral-200 hover:border-sky-500"
      >
        {icon ? (
          <IconView icon={icon} size={26} />
        ) : (
          <span className="text-xs text-neutral-500">—</span>
        )}
      </button>
      {open && (
        <IconPicker
          onClose={() => setOpen(false)}
          onPick={(i) => {
            onPick(i);
            setOpen(false);
          }}
        />
      )}
    </>
  );
}

export function ButtonInspector({
  button,
  onChange,
  onDelete,
  onClose,
  onOpenFolder,
  ctx,
  extra,
}: {
  button: Button;
  onChange: (b: Button) => void;
  onDelete: () => void;
  onClose: () => void;
  onOpenFolder?: () => void;
  ctx: FormContext;
  /** Replaces the action form (plugin Property Inspector). */
  extra?: ReactNode;
}) {
  const t = useT();
  const [testResult, setTestResult] = useState<{ ok: boolean; msg: string } | null>(null);
  const [testing, setTesting] = useState(false);
  const errors = validateButton(button);
  const showStateB = hasStateB(button.action);
  const stateB: Visual = button.stateB ?? { title: "", icon: null };

  const test = async () => {
    setTesting(true);
    setTestResult(null);
    try {
      await api.testButton(button);
      setTestResult({ ok: true, msg: t("inspector.testOk") });
    } catch (e) {
      setTestResult({ ok: false, msg: String(e) });
    } finally {
      setTesting(false);
    }
  };

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center justify-between border-b border-neutral-800 px-4 py-3">
        <h2 className="text-sm font-semibold text-neutral-200">
          {t(`type.${button.action.type}`)}
        </h2>
        <button
          className="text-neutral-500 hover:text-white"
          onClick={onClose}
          title={t("common.close")}
        >
          <X size={18} />
        </button>
      </div>
      <div className="min-h-0 flex-1 space-y-5 overflow-y-auto p-4">
        <div className="flex items-end gap-3">
          <IconButton icon={button.icon} onPick={(icon) => onChange({ ...button, icon })} />
          <div className="flex-1">
            <Field label={t("inspector.title")}>
              <TextInput
                value={button.title}
                onChange={(title) => onChange({ ...button, title })}
              />
            </Field>
          </div>
        </div>

        {showStateB && (
          <div className="rounded-lg border border-neutral-800 p-3">
            <p className="mb-2 text-xs font-medium text-neutral-400">{t("inspector.stateB")}</p>
            <div className="flex items-end gap-3">
              <IconButton
                icon={stateB.icon}
                onPick={(icon) => onChange({ ...button, stateB: { ...stateB, icon } })}
              />
              <div className="flex-1">
                <Field label={t("inspector.title")}>
                  <TextInput
                    value={stateB.title}
                    onChange={(title) => onChange({ ...button, stateB: { ...stateB, title } })}
                  />
                </Field>
              </div>
            </div>
          </div>
        )}

        {button.action.type === "folder" ? (
          <Btn onClick={onOpenFolder}>
            <FolderOpen size={15} /> {t("inspector.openFolder")}
          </Btn>
        ) : (
          (extra ?? (
            <ActionForm
              action={button.action}
              onChange={(action) => onChange({ ...button, action })}
              ctx={ctx}
            />
          ))
        )}

        {errors.length > 0 && (
          <ul className="space-y-1 text-xs text-amber-400">
            {errors.map((e) => (
              <li key={e}>• {t(e)}</li>
            ))}
          </ul>
        )}
      </div>
      <div className="space-y-2 border-t border-neutral-800 p-3">
        {testResult && (
          <p
            className={
              testResult.ok ? "text-xs text-emerald-400" : "text-xs break-words text-rose-400"
            }
          >
            {testResult.msg}
          </p>
        )}
        <div className="flex gap-2">
          <Btn
            variant="primary"
            onClick={test}
            disabled={testing || errors.length > 0 || button.action.type === "folder"}
            title={t("inspector.testHint")}
          >
            <Play size={15} /> {t("inspector.test")}
          </Btn>
          <div className="flex-1" />
          <Btn variant="danger" onClick={onDelete}>
            <Trash2 size={15} /> {t("common.delete")}
          </Btn>
        </div>
      </div>
    </div>
  );
}
