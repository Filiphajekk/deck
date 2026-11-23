// Icon picker: built-in Lucide icons, custom image, emoji or icon of an app.
import { open } from "@tauri-apps/plugin-dialog";
import { useMemo, useState } from "react";
import { IconView } from "../components/PanelView";
import { useT } from "../i18n";
import { api } from "../lib/api";
import { ICON_NAMES, ICONS } from "../lib/icons";
import type { Icon } from "../types/profile";
import { AppPicker } from "./AppPicker";
import { Btn, cx, Modal, TextInput } from "./ui";

type Tab = "builtin" | "image" | "emoji" | "app";

const EMOJI = [
  "😀",
  "😎",
  "🤖",
  "👾",
  "🎮",
  "🎧",
  "🎵",
  "🎤",
  "📷",
  "🎬",
  "📺",
  "💬",
  "🔥",
  "⭐",
  "❤️",
  "✅",
  "❌",
  "⚡",
  "💡",
  "🔒",
  "🔔",
  "📌",
  "📁",
  "🌐",
  "🚀",
  "🏆",
  "🎯",
  "🧠",
  "☕",
  "🍕",
  "🐱",
  "🐶",
  "👍",
  "👋",
  "🙏",
  "💤",
];

export function IconPicker({
  onPick,
  onClose,
}: {
  onPick: (icon: Icon | null) => void;
  onClose: () => void;
}) {
  const t = useT();
  const [tab, setTab] = useState<Tab>("builtin");
  const [query, setQuery] = useState("");
  const [emoji, setEmoji] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return q ? ICON_NAMES.filter((n) => n.includes(q)) : ICON_NAMES;
  }, [query]);

  const pickImage = async () => {
    setError(null);
    const path = await open({
      multiple: false,
      filters: [
        { name: t("icon.images"), extensions: ["png", "jpg", "jpeg", "svg", "gif", "webp"] },
      ],
    });
    if (typeof path !== "string") return;
    setBusy(true);
    try {
      onPick({ kind: "image", src: await api.loadImage(path) });
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const pickExeFile = async () => {
    setError(null);
    const path = await open({
      multiple: false,
      filters: [{ name: t("icon.programs"), extensions: ["exe", "lnk", "ico", "dll"] }],
    });
    if (typeof path !== "string") return;
    await fromPath(path);
  };

  const fromPath = async (path: string) => {
    setBusy(true);
    try {
      onPick({ kind: "image", src: await api.fileIcon(path) });
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const tabs: { id: Tab; label: string }[] = [
    { id: "builtin", label: t("icon.builtin") },
    { id: "image", label: t("icon.image") },
    { id: "emoji", label: t("icon.emoji") },
    { id: "app", label: t("icon.app") },
  ];

  return (
    <Modal
      title={t("icon.title")}
      onClose={onClose}
      wide
      footer={
        <>
          <Btn variant="ghost" onClick={() => onPick(null)}>
            {t("icon.none")}
          </Btn>
          <Btn onClick={onClose}>{t("common.close")}</Btn>
        </>
      }
    >
      <div className="mb-3 flex gap-1">
        {tabs.map((x) => (
          <button
            key={x.id}
            onClick={() => setTab(x.id)}
            className={cx(
              "rounded-md px-3 py-1 text-sm",
              tab === x.id ? "bg-neutral-700 text-white" : "text-neutral-400 hover:text-white",
            )}
          >
            {x.label}
          </button>
        ))}
      </div>
      {error && <p className="mb-2 text-sm text-rose-400">{error}</p>}
      {tab === "builtin" && (
        <div className="space-y-3">
          <TextInput value={query} onChange={setQuery} placeholder={t("common.search")} autoFocus />
          <div className="grid max-h-[50vh] grid-cols-10 gap-1 overflow-y-auto">
            {filtered.map((name) => {
              const C = ICONS[name];
              return (
                <button
                  key={name}
                  title={name}
                  onClick={() => onPick({ kind: "lucide", name })}
                  className="flex aspect-square items-center justify-center rounded-md text-neutral-300 hover:bg-neutral-700 hover:text-white"
                >
                  <C size={20} />
                </button>
              );
            })}
          </div>
        </div>
      )}
      {tab === "image" && (
        <div className="space-y-2">
          <p className="text-sm text-neutral-400">{t("icon.imageHint")}</p>
          <Btn variant="primary" onClick={pickImage} disabled={busy}>
            {t("icon.chooseFile")}
          </Btn>
        </div>
      )}
      {tab === "emoji" && (
        <div className="space-y-3">
          <div className="flex gap-2">
            <TextInput
              value={emoji}
              onChange={setEmoji}
              placeholder="🙂"
              className="w-32 text-xl"
            />
            <Btn
              variant="primary"
              disabled={!emoji.trim()}
              onClick={() => onPick({ kind: "emoji", value: emoji.trim() })}
            >
              {t("common.use")}
            </Btn>
          </div>
          <div className="grid grid-cols-12 gap-1">
            {EMOJI.map((e) => (
              <button
                key={e}
                className="rounded-md p-1 text-2xl hover:bg-neutral-700"
                onClick={() => onPick({ kind: "emoji", value: e })}
              >
                <IconView icon={{ kind: "emoji", value: e }} size={24} />
              </button>
            ))}
          </div>
        </div>
      )}
      {tab === "app" && (
        <div className="space-y-3">
          <Btn onClick={pickExeFile} disabled={busy}>
            {t("icon.chooseExe")}
          </Btn>
          <AppPicker onPick={(app) => fromPath(app.path)} />
        </div>
      )}
    </Modal>
  );
}
