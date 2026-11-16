import { Gamepad2, Keyboard, ListPlus, Palette, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useI18n } from "../i18n";
import { getBackend } from "../lib/backend";
import { hexToRgb, rgbCss, rgbToHex } from "../lib/color";
import type { Profile, Rgb } from "../lib/types";

export function ProfileEditor({ initial, currentKeys, onSave, onClose }: {
  initial: Profile | null;
  /** Per-key colours of the connected keyboard, if any. */
  currentKeys: Record<string, Rgb> | null;
  onSave: (p: Profile) => void;
  onClose: () => void;
}) {
  const { t } = useI18n();
  const ref = useRef<HTMLDialogElement>(null);
  const [draft, setDraft] = useState<Profile | null>(initial);
  const [game, setGame] = useState("");
  const [running, setRunning] = useState<string[] | null>(null);

  useEffect(() => {
    setDraft(initial);
    setRunning(null);
    setGame("");
    const d = ref.current;
    if (!d) return;
    if (initial && !d.open) d.showModal();
    if (!initial && d.open) d.close();
  }, [initial]);

  if (!draft) return <dialog ref={ref} className="hidden" />;
  const set = (patch: Partial<Profile>) => setDraft({ ...draft, ...patch });
  const addGame = (g: string) => {
    const name = g.trim().toLowerCase();
    if (name && !draft.games.includes(name)) set({ games: [...draft.games, name] });
    setGame("");
  };
  const keyCount = Object.keys(draft.keys).length;

  return (
    <dialog
      ref={ref}
      onClose={onClose}
      onClick={(e) => e.target === ref.current && onClose()}
      className="m-auto w-[min(520px,calc(100vw-32px))] rounded-2xl border border-line bg-surface p-0 text-text shadow-2xl shadow-black/60 backdrop:bg-black/60 backdrop:backdrop-blur-sm"
    >
      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (draft.name.trim()) onSave({ ...draft, name: draft.name.trim() });
        }}
      >
        <header className="flex items-center gap-2 border-b border-line px-5 py-4">
          <h2 className="text-sm font-semibold">{draft.name || t("prof.new")}</h2>
          <button type="button" onClick={onClose} aria-label={t("prof.cancel")} className="ml-auto rounded-lg p-1.5 text-muted hover:bg-surface-2 hover:text-text">
            <X size={16} />
          </button>
        </header>

        <div className="flex flex-col gap-5 p-5 text-sm">
          <label className="flex flex-col gap-1.5">
            <span className="text-xs font-semibold text-muted">{t("prof.name")}</span>
            <input
              required
              autoFocus
              value={draft.name}
              onChange={(e) => set({ name: e.target.value })}
              className="rounded-lg border border-line bg-bg px-3 py-2 outline-none focus:border-accent"
            />
          </label>

          <section className="flex flex-col gap-2">
            <span className="flex items-center gap-2 text-xs font-semibold text-muted">
              <Gamepad2 size={14} /> {t("prof.games")}
            </span>
            <div className="flex flex-wrap gap-1.5">
              {draft.games.map((g) => (
                <span key={g} className="flex items-center gap-1 rounded-full border border-line bg-bg py-0.5 pr-1 pl-2.5 font-mono text-[11px]">
                  {g}
                  <button type="button" onClick={() => set({ games: draft.games.filter((x) => x !== g) })} className="rounded-full p-0.5 text-muted hover:text-rose-300" aria-label={t("prof.delete")}>
                    <X size={11} />
                  </button>
                </span>
              ))}
            </div>
            <div className="flex gap-2">
              <input
                value={game}
                onChange={(e) => setGame(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    addGame(game);
                  }
                }}
                placeholder={t("prof.gamePlaceholder")}
                className="flex-1 rounded-lg border border-line bg-bg px-3 py-2 font-mono text-xs outline-none focus:border-accent"
              />
              <button type="button" onClick={() => addGame(game)} className="rounded-lg border border-line px-3 text-xs text-muted hover:border-accent/60 hover:text-text">
                {t("prof.addGame")}
              </button>
              <button
                type="button"
                title={t("prof.running")}
                aria-label={t("prof.running")}
                onClick={async () => setRunning(await (await getBackend()).runningProcesses())}
                className="rounded-lg border border-line px-2.5 text-muted hover:border-accent/60 hover:text-text"
              >
                <ListPlus size={15} />
              </button>
            </div>
            {running && (
              <ul className="max-h-36 overflow-y-auto rounded-lg border border-line bg-bg p-1 font-mono text-[11px]">
                {running.map((p) => (
                  <li key={p}>
                    <button type="button" onClick={() => addGame(p)} className="w-full rounded px-2 py-1 text-left hover:bg-accent/20">
                      {p}
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </section>

          <section className="flex items-center gap-3">
            <Palette size={14} className="text-muted" />
            <label className="flex flex-1 items-center gap-2">
              <input
                type="checkbox"
                checked={draft.color != null}
                onChange={(e) => set({ color: e.target.checked ? (draft.color ?? { r: 124, g: 92, b: 255 }) : null })}
                className="accent-[var(--color-accent)]"
              />
              <span>{draft.color ? t("prof.color") : t("prof.noColor")}</span>
            </label>
            {draft.color && (
              <input
                type="color"
                value={rgbToHex(draft.color)}
                onChange={(e) => set({ color: hexToRgb(e.target.value) })}
                className="h-8 w-12 cursor-pointer rounded border border-line bg-transparent"
                style={{ background: rgbCss(draft.color) }}
              />
            )}
          </section>

          <section className="flex flex-wrap items-center gap-2">
            <Keyboard size={14} className="text-muted" />
            <span className="mr-auto">
              {t("prof.keys")}: <b>{keyCount || "—"}</b>
            </span>
            {currentKeys && (
              <button type="button" onClick={() => set({ keys: { ...currentKeys } })} className="rounded-lg border border-line px-2.5 py-1 text-[11px] text-muted hover:border-accent/60 hover:text-text">
                {t("prof.useCurrentKeys")}
              </button>
            )}
            {keyCount > 0 && (
              <button type="button" onClick={() => set({ keys: {} })} className="rounded-lg border border-line px-2.5 py-1 text-[11px] text-muted hover:border-rose-400/60 hover:text-rose-300">
                {t("prof.clearKeys")}
              </button>
            )}
          </section>
        </div>

        <footer className="flex justify-end gap-2 border-t border-line px-5 py-4">
          <button type="button" onClick={onClose} className="rounded-lg border border-line px-4 py-2 text-xs text-muted hover:text-text">
            {t("prof.cancel")}
          </button>
          <button type="submit" className="rounded-lg bg-accent px-4 py-2 text-xs font-semibold text-white hover:brightness-110">
            {t("prof.save")}
          </button>
        </footer>
      </form>
    </dialog>
  );
}
