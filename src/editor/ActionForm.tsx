// Parameter form for every action type. `multi` and `toggle` nest this form recursively.
import { open } from "@tauri-apps/plugin-dialog";
import { ArrowDown, ArrowUp, FolderOpen, Plus, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { useT } from "../i18n";
import { api, type AudioDevice } from "../lib/api";
import type { Action, ActionType, KeyCombo, Profile } from "../types/profile";
import { AppPicker } from "./AppPicker";
import { defaultAction, SUB_ACTION_TYPES } from "./catalog";
import { HotkeyInput } from "./HotkeyInput";
import { Btn, Field, Modal, NumberInput, Select, TextArea, TextInput, Toggle } from "./ui";

export interface FormContext {
  profiles: Profile[];
}

type Props<T extends Action = Action> = {
  action: T;
  onChange: (a: Action) => void;
  ctx: FormContext;
  depth?: number;
};

export function ActionForm({ action, onChange, ctx, depth = 0 }: Props) {
  switch (action.type) {
    case "hotkey":
      return <HotkeyForm action={action} onChange={onChange} ctx={ctx} />;
    case "typeText":
      return <TypeTextForm action={action} onChange={onChange} ctx={ctx} />;
    case "launch":
      return <LaunchForm action={action} onChange={onChange} ctx={ctx} />;
    case "open":
      return <OpenForm action={action} onChange={onChange} ctx={ctx} />;
    case "command":
      return <CommandForm action={action} onChange={onChange} ctx={ctx} />;
    case "media":
      return <MediaForm action={action} onChange={onChange} ctx={ctx} />;
    case "volume":
      return <VolumeForm action={action} onChange={onChange} ctx={ctx} />;
    case "appVolume":
      return <AppVolumeForm action={action} onChange={onChange} ctx={ctx} />;
    case "audioDevice":
      return <AudioDeviceForm action={action} onChange={onChange} ctx={ctx} />;
    case "window":
      return <WindowForm action={action} onChange={onChange} ctx={ctx} />;
    case "system":
      return <SystemForm action={action} onChange={onChange} ctx={ctx} />;
    case "obs":
      return <ObsForm action={action} onChange={onChange} ctx={ctx} />;
    case "multi":
      return <MultiForm action={action} onChange={onChange} ctx={ctx} depth={depth} />;
    case "toggle":
      return <ToggleForm action={action} onChange={onChange} ctx={ctx} depth={depth} />;
    case "switchProfile":
      return <SwitchProfileForm action={action} onChange={onChange} ctx={ctx} />;
    case "delay":
      return <DelayForm action={action} onChange={onChange} ctx={ctx} />;
    default:
      return <NoParams />;
  }
}

function NoParams() {
  const t = useT();
  return <p className="text-sm text-neutral-500">{t("form.noParams")}</p>;
}

type A<T extends ActionType> = Extract<Action, { type: T }>;

function HotkeyForm({ action, onChange }: Props<A<"hotkey">>) {
  const t = useT();
  const keys = action.keys.length ? action.keys : [];
  const set = (i: number, c: KeyCombo) => {
    const next = keys.slice();
    next[i] = c;
    onChange({ ...action, keys: next });
  };
  return (
    <div className="space-y-2">
      <Field label={t("form.shortcut")} hint={t("form.shortcutHint")}>
        <div className="space-y-2">
          {keys.map((c, i) => (
            <div key={i} className="flex items-center gap-1">
              <div className="flex-1">
                <HotkeyInput value={c} onChange={(v) => set(i, v)} />
              </div>
              {keys.length > 1 && (
                <button
                  className="p-1 text-neutral-500 hover:text-rose-400"
                  onClick={() => onChange({ ...action, keys: keys.filter((_, j) => j !== i) })}
                >
                  <Trash2 size={15} />
                </button>
              )}
            </div>
          ))}
          {keys.length === 0 && <HotkeyInput value={null} onChange={(v) => set(0, v)} />}
        </div>
      </Field>
      {keys.length > 0 && (
        <Btn
          small
          variant="ghost"
          onClick={() =>
            onChange({
              ...action,
              keys: [...keys, { ctrl: false, shift: false, alt: false, win: false, key: null }],
            })
          }
        >
          <Plus size={14} /> {t("form.addCombo")}
        </Btn>
      )}
    </div>
  );
}

function TypeTextForm({ action, onChange }: Props<A<"typeText">>) {
  const t = useT();
  return (
    <Field label={t("form.text")}>
      <TextArea rows={4} value={action.text} onChange={(text) => onChange({ ...action, text })} />
    </Field>
  );
}

function BrowseButton({
  onPath,
  directory,
  filters,
}: {
  onPath: (p: string) => void;
  directory?: boolean;
  filters?: { name: string; extensions: string[] }[];
}) {
  return (
    <Btn
      onClick={async () => {
        const p = await open({ multiple: false, directory, filters });
        if (typeof p === "string") onPath(p);
      }}
    >
      <FolderOpen size={15} />
    </Btn>
  );
}

function LaunchForm({ action, onChange }: Props<A<"launch">>) {
  const t = useT();
  const [picker, setPicker] = useState(false);
  return (
    <div className="space-y-3">
      <Field label={t("form.program")}>
        <div className="flex gap-1">
          <TextInput value={action.path} onChange={(path) => onChange({ ...action, path })} />
          <BrowseButton onPath={(path) => onChange({ ...action, path })} />
        </div>
      </Field>
      <Btn small variant="ghost" onClick={() => setPicker(true)}>
        {t("form.pickApp")}
      </Btn>
      <Field label={t("form.args")}>
        <TextInput value={action.args} onChange={(args) => onChange({ ...action, args })} />
      </Field>
      <Field label={t("form.workingDir")}>
        <div className="flex gap-1">
          <TextInput
            value={action.workingDir}
            onChange={(workingDir) => onChange({ ...action, workingDir })}
          />
          <BrowseButton directory onPath={(workingDir) => onChange({ ...action, workingDir })} />
        </div>
      </Field>
      <Toggle
        checked={action.admin}
        onChange={(admin) => onChange({ ...action, admin })}
        label={t("form.runAsAdmin")}
      />
      {picker && (
        <Modal title={t("form.pickApp")} onClose={() => setPicker(false)} wide>
          <AppPicker
            onPick={(app) => {
              onChange({ ...action, path: app.path });
              setPicker(false);
            }}
          />
        </Modal>
      )}
    </div>
  );
}

function OpenForm({ action, onChange }: Props<A<"open">>) {
  const t = useT();
  return (
    <Field label={t("form.target")} hint={t("form.targetHint")}>
      <div className="flex gap-1">
        <TextInput value={action.target} onChange={(target) => onChange({ ...action, target })} />
        <BrowseButton onPath={(target) => onChange({ ...action, target })} />
        <BrowseButton directory onPath={(target) => onChange({ ...action, target })} />
      </div>
    </Field>
  );
}

function CommandForm({ action, onChange }: Props<A<"command">>) {
  const t = useT();
  return (
    <div className="space-y-3">
      <Field label={t("form.command")}>
        <TextArea value={action.command} onChange={(command) => onChange({ ...action, command })} />
      </Field>
      <Field label={t("form.shell")}>
        <Select
          value={action.shell}
          onChange={(shell) => onChange({ ...action, shell })}
          options={[
            { value: "cmd", label: "cmd" },
            { value: "powershell", label: "PowerShell" },
          ]}
        />
      </Field>
      <Toggle
        checked={action.hidden}
        onChange={(hidden) => onChange({ ...action, hidden })}
        label={t("form.hideWindow")}
      />
    </div>
  );
}

function MediaForm({ action, onChange }: Props<A<"media">>) {
  const t = useT();
  return (
    <Field label={t("form.operation")}>
      <Select
        value={action.op}
        onChange={(op) => onChange({ ...action, op })}
        options={(["playPause", "next", "previous", "stop"] as const).map((v) => ({
          value: v,
          label: t(`media.${v}`),
        }))}
      />
    </Field>
  );
}

function VolumeForm({ action, onChange }: Props<A<"volume">>) {
  const t = useT();
  return (
    <div className="space-y-3">
      <Field label={t("form.operation")}>
        <Select
          value={action.op}
          onChange={(op) => onChange({ ...action, op })}
          options={(["up", "down", "set", "muteToggle"] as const).map((v) => ({
            value: v,
            label: t(`volume.${v}`),
          }))}
        />
      </Field>
      {(action.op === "up" || action.op === "down") && (
        <Field label={t("form.stepPercent")}>
          <NumberInput
            min={1}
            max={100}
            value={action.step}
            onChange={(step) => onChange({ ...action, step })}
          />
        </Field>
      )}
      {action.op === "set" && (
        <Field label={t("form.valuePercent")}>
          <NumberInput
            min={0}
            max={100}
            value={action.value}
            onChange={(value) => onChange({ ...action, value })}
          />
        </Field>
      )}
    </div>
  );
}

function AppVolumeForm({ action, onChange }: Props<A<"appVolume">>) {
  const t = useT();
  const [apps, setApps] = useState<string[]>([]);
  useEffect(() => {
    api.audioApps().then(setApps);
  }, []);
  return (
    <div className="space-y-3">
      <Field label={t("form.appExe")} hint={t("form.appExeHint")}>
        <TextInput
          value={action.exe}
          list="audio-apps"
          placeholder="spotify.exe"
          onChange={(exe) => onChange({ ...action, exe })}
        />
        <datalist id="audio-apps">
          {apps.map((a) => (
            <option key={a} value={a} />
          ))}
        </datalist>
      </Field>
      <Field label={t("form.operation")}>
        <Select
          value={action.op === "set" ? "muteToggle" : action.op}
          onChange={(op) => onChange({ ...action, op })}
          options={(["up", "down", "muteToggle"] as const).map((v) => ({
            value: v,
            label: t(`volume.${v}`),
          }))}
        />
      </Field>
      {action.op !== "muteToggle" && (
        <Field label={t("form.stepPercent")}>
          <NumberInput
            min={1}
            max={100}
            value={action.step}
            onChange={(step) => onChange({ ...action, step })}
          />
        </Field>
      )}
    </div>
  );
}

function AudioDeviceForm({ action, onChange }: Props<A<"audioDevice">>) {
  const t = useT();
  const [devices, setDevices] = useState<AudioDevice[]>([]);
  useEffect(() => {
    api
      .audioDevices(action.flow)
      .then(setDevices)
      .catch(() => setDevices([]));
  }, [action.flow]);
  const CYCLE = "__cycle__";
  return (
    <div className="space-y-3">
      <Field label={t("form.deviceKind")}>
        <Select
          value={action.flow}
          onChange={(flow) => onChange({ ...action, flow, deviceId: null, deviceName: "" })}
          options={[
            { value: "output", label: t("audio.output") },
            { value: "input", label: t("audio.input") },
          ]}
        />
      </Field>
      <Field label={t("form.device")}>
        <Select
          value={action.deviceId ?? CYCLE}
          onChange={(id) => {
            if (id === CYCLE) onChange({ ...action, deviceId: null, deviceName: "" });
            else
              onChange({
                ...action,
                deviceId: id,
                deviceName: devices.find((d) => d.id === id)?.name ?? "",
              });
          }}
          options={[
            { value: CYCLE, label: t("audio.cycle") },
            ...devices.map((d) => ({ value: d.id, label: d.name })),
            ...(action.deviceId && !devices.some((d) => d.id === action.deviceId)
              ? [
                  {
                    value: action.deviceId,
                    label: `${action.deviceName} (${t("audio.unavailable")})`,
                  },
                ]
              : []),
          ]}
        />
      </Field>
    </div>
  );
}

function WindowForm({ action, onChange }: Props<A<"window">>) {
  const t = useT();
  return (
    <Field label={t("form.operation")}>
      <Select
        value={action.op}
        onChange={(op) => onChange({ ...action, op })}
        options={(["minimize", "maximize", "nextMonitor", "topmostToggle", "close"] as const).map(
          (v) => ({ value: v, label: t(`window.${v}`) }),
        )}
      />
    </Field>
  );
}

function SystemForm({ action, onChange }: Props<A<"system">>) {
  const t = useT();
  return (
    <Field label={t("form.operation")}>
      <Select
        value={action.op}
        onChange={(op) => onChange({ ...action, op })}
        options={(["lock", "sleep", "snip"] as const).map((v) => ({
          value: v,
          label: t(`system.${v}`),
        }))}
      />
    </Field>
  );
}

function ObsForm({ action, onChange }: Props<A<"obs">>) {
  const t = useT();
  const [scenes, setScenes] = useState<string[]>([]);
  const [sources, setSources] = useState<string[]>([]);
  useEffect(() => {
    import("@tauri-apps/api/core").then(({ invoke }) => {
      invoke<{ scenes: string[]; inputs: string[] }>("obs_lists")
        .then((l) => {
          setScenes(l.scenes);
          setSources(l.inputs);
        })
        .catch(() => {});
    });
  }, []);
  return (
    <div className="space-y-3">
      <Field label={t("form.operation")}>
        <Select
          value={action.op}
          onChange={(op) => onChange({ ...action, op })}
          options={(
            [
              "switchScene",
              "toggleRecord",
              "startRecord",
              "stopRecord",
              "toggleStream",
              "startStream",
              "stopStream",
              "toggleMute",
              "saveReplay",
            ] as const
          ).map((v) => ({ value: v, label: t(`obs.${v}`) }))}
        />
      </Field>
      {action.op === "switchScene" && (
        <Field label={t("form.scene")}>
          <TextInput
            value={action.scene}
            list="obs-scenes"
            onChange={(scene) => onChange({ ...action, scene })}
          />
          <datalist id="obs-scenes">
            {scenes.map((s) => (
              <option key={s} value={s} />
            ))}
          </datalist>
        </Field>
      )}
      {action.op === "toggleMute" && (
        <Field label={t("form.source")}>
          <TextInput
            value={action.source}
            list="obs-sources"
            onChange={(source) => onChange({ ...action, source })}
          />
          <datalist id="obs-sources">
            {sources.map((s) => (
              <option key={s} value={s} />
            ))}
          </datalist>
        </Field>
      )}
      <p className="text-xs text-neutral-500">{t("form.obsHint")}</p>
    </div>
  );
}

function SubAction({
  action,
  onChange,
  onRemove,
  onMove,
  ctx,
  depth,
}: {
  action: Action;
  onChange: (a: Action) => void;
  onRemove?: () => void;
  onMove?: (d: -1 | 1) => void;
  ctx: FormContext;
  depth: number;
}) {
  const t = useT();
  return (
    <div className="space-y-2 rounded-lg border border-neutral-800 bg-neutral-900/60 p-2.5">
      <div className="flex items-center gap-1">
        <div className="flex-1">
          <Select
            value={action.type}
            onChange={(type) => onChange(defaultAction(type))}
            options={SUB_ACTION_TYPES.map((v) => ({ value: v, label: t(`type.${v}`) }))}
          />
        </div>
        {onMove && (
          <>
            <button className="p-1 text-neutral-500 hover:text-white" onClick={() => onMove(-1)}>
              <ArrowUp size={15} />
            </button>
            <button className="p-1 text-neutral-500 hover:text-white" onClick={() => onMove(1)}>
              <ArrowDown size={15} />
            </button>
          </>
        )}
        {onRemove && (
          <button className="p-1 text-neutral-500 hover:text-rose-400" onClick={onRemove}>
            <Trash2 size={15} />
          </button>
        )}
      </div>
      <ActionForm action={action} onChange={onChange} ctx={ctx} depth={depth + 1} />
    </div>
  );
}

function MultiForm({ action, onChange, ctx, depth = 0 }: Props<A<"multi">>) {
  const t = useT();
  const set = (i: number, a: Action) => {
    const next = action.actions.slice();
    next[i] = a;
    onChange({ ...action, actions: next });
  };
  const move = (i: number, d: -1 | 1) => {
    const j = i + d;
    if (j < 0 || j >= action.actions.length) return;
    const next = action.actions.slice();
    [next[i], next[j]] = [next[j], next[i]];
    onChange({ ...action, actions: next });
  };
  return (
    <div className="space-y-2">
      <p className="text-xs text-neutral-500">{t("form.multiHint")}</p>
      {action.actions.map((a, i) => (
        <SubAction
          key={i}
          action={a}
          ctx={ctx}
          depth={depth}
          onChange={(x) => set(i, x)}
          onMove={(d) => move(i, d)}
          onRemove={() =>
            onChange({ ...action, actions: action.actions.filter((_, j) => j !== i) })
          }
        />
      ))}
      <div className="flex gap-1">
        <Btn
          small
          onClick={() =>
            onChange({ ...action, actions: [...action.actions, defaultAction("hotkey")] })
          }
        >
          <Plus size={14} /> {t("form.addStep")}
        </Btn>
        <Btn
          small
          variant="ghost"
          onClick={() =>
            onChange({ ...action, actions: [...action.actions, { type: "delay", ms: 200 }] })
          }
        >
          <Plus size={14} /> {t("type.delay")}
        </Btn>
      </div>
    </div>
  );
}

function ToggleForm({ action, onChange, ctx, depth = 0 }: Props<A<"toggle">>) {
  const t = useT();
  return (
    <div className="space-y-3">
      <p className="text-xs text-neutral-500">{t("form.toggleHint")}</p>
      <Field label={t("form.toggleA")}>
        <SubAction
          action={action.a}
          ctx={ctx}
          depth={depth}
          onChange={(a) => onChange({ ...action, a })}
        />
      </Field>
      <Field label={t("form.toggleB")}>
        <SubAction
          action={action.b}
          ctx={ctx}
          depth={depth}
          onChange={(b) => onChange({ ...action, b })}
        />
      </Field>
    </div>
  );
}

function SwitchProfileForm({ action, onChange, ctx }: Props<A<"switchProfile">>) {
  const t = useT();
  return (
    <Field label={t("form.profile")}>
      <Select
        value={action.profileId}
        onChange={(profileId) => onChange({ ...action, profileId })}
        options={[
          { value: "", label: "—" },
          ...ctx.profiles.map((p) => ({ value: p.id, label: p.name })),
        ]}
      />
    </Field>
  );
}

function DelayForm({ action, onChange }: Props<A<"delay">>) {
  const t = useT();
  return (
    <Field label={t("form.delayMs")}>
      <NumberInput
        min={0}
        max={60000}
        step={50}
        value={action.ms}
        onChange={(ms) => onChange({ ...action, ms })}
      />
    </Field>
  );
}
