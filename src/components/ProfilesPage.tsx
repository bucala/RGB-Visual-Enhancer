import { Gamepad2, Info, Pencil, Play, Plus, Star, Trash2 } from "lucide-react";
import { useState } from "react";
import { useI18n } from "../i18n";
import { rgbCss } from "../lib/color";
import { TEMPLATES } from "../lib/templates";
import type { Profile, Rgb } from "../lib/types";
import { useProfiles, newProfileId } from "../hooks/useProfiles";
import { useContextMenu } from "./ContextMenu";
import { Toggle } from "./ui";

export function ProfilesPage({ profiles, onEdit, keyboardKeys }: {
  profiles: ReturnType<typeof useProfiles>;
  onEdit: (p: Profile) => void;
  keyboardKeys: Record<string, Rgb> | null;
}) {
  const { t } = useI18n();
  const ctx = useContextMenu();
  const { view, apply, remove, setDefault, setAutoSwitch } = profiles;
  const [confirm, setConfirm] = useState<string | null>(null);

  const create = (base?: Partial<Profile>) =>
    onEdit({ id: newProfileId(), name: "", games: [], color: null, keys: keyboardKeys ?? {}, ...base });

  return (
    <div className="mx-auto flex max-w-5xl flex-col gap-5">
      <div className="flex flex-wrap items-end gap-4">
        <div className="mr-auto">
          <h2 className="text-lg font-semibold">{t("nav.profiles")}</h2>
          <p className="text-xs text-muted">{t("prof.subtitle")}</p>
        </div>
        <Toggle label={t("prof.autoSwitch")} checked={view.auto_switch} onChange={setAutoSwitch} />
        <button type="button" onClick={() => create()} className="flex items-center gap-2 rounded-lg bg-accent px-3.5 py-2 text-xs font-semibold text-white hover:brightness-110">
          <Plus size={14} /> {t("prof.new")}
        </button>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <span className="text-[11px] text-muted">{t("kb.templates")}:</span>
        {TEMPLATES.map((tpl) => (
          <button
            key={tpl.id}
            type="button"
            onClick={() => create({ name: t(tpl.label), keys: tpl.keys(), games: tpl.games })}
            className="flex items-center gap-1.5 rounded-full border border-line px-2.5 py-1 text-[11px] text-muted transition hover:border-accent/50 hover:text-text"
          >
            <Gamepad2 size={12} /> {t(tpl.label)}
          </button>
        ))}
      </div>

      {view.profiles.length === 0 && (
        <div className="rounded-2xl border border-dashed border-line p-8 text-center text-sm text-muted">{t("prof.empty")}</div>
      )}

      <ul className="grid gap-3 md:grid-cols-2">
        {view.profiles.map((p) => {
          const active = view.active.id === p.id;
          const isDefault = view.default_profile === p.id;
          const swatches = Object.values(p.keys).filter((c) => c.r || c.g || c.b);
          const unique = [...new Map(swatches.map((c) => [rgbCss(c), c])).values()].slice(0, 12);
          return (
            <li
              key={p.id}
              className={`flex flex-col gap-3 rounded-2xl border bg-surface p-4 transition ${active ? "border-accent/70 shadow-lg shadow-accent/10" : "border-line"}`}
              onContextMenu={(e) =>
                ctx.open(e, [
                  { kind: "header", label: p.name },
                  { kind: "item", label: t("prof.apply"), icon: <Play size={13} />, onSelect: () => apply(p.id) },
                  { kind: "item", label: t("prof.edit"), icon: <Pencil size={13} />, onSelect: () => onEdit(p) },
                  { kind: "item", label: t("prof.makeDefault"), icon: <Star size={13} />, checked: isDefault, onSelect: () => setDefault(isDefault ? null : p.id) },
                  { kind: "separator" },
                  { kind: "item", label: t("prof.delete"), icon: <Trash2 size={13} />, danger: true, onSelect: () => remove(p.id) },
                ])
              }
            >
              <div className="flex items-start gap-2">
                <div className="min-w-0 flex-1">
                  <h3 dir="auto" className="truncate rtl:text-right font-semibold">{p.name}</h3>
                  <div className="mt-1 flex flex-wrap gap-1.5 text-[10px]">
                    {active && (
                      <span className="rounded-full bg-accent/20 px-2 py-0.5 font-semibold text-accent">
                        {view.active.game ? t("prof.activeGame", { game: view.active.game }) : t("prof.active")}
                      </span>
                    )}
                    {isDefault && <span className="rounded-full bg-amber-400/15 px-2 py-0.5 font-semibold text-amber-300">{t("prof.default")}</span>}
                  </div>
                </div>
                <button
                  type="button"
                  onClick={() => setDefault(isDefault ? null : p.id)}
                  title={t("prof.makeDefault")}
                  aria-label={t("prof.makeDefault")}
                  className={`rounded-lg p-1.5 transition ${isDefault ? "text-amber-300" : "text-muted hover:text-amber-200"}`}
                >
                  <Star size={16} fill={isDefault ? "currentColor" : "none"} />
                </button>
              </div>

              <div className="flex flex-wrap gap-1.5">
                {p.games.map((g) => (
                  <span key={g} className="flex items-center gap-1 rounded-full border border-line bg-bg px-2 py-0.5 font-mono text-[10px] text-muted">
                    <Gamepad2 size={10} /> {g}
                  </span>
                ))}
              </div>

              <div className="flex items-center gap-1">
                {p.color && <span className="mr-1 h-5 w-5 rounded-md border border-white/20" style={{ background: rgbCss(p.color) }} />}
                {unique.map((c, i) => (
                  <span key={i} className="h-3 w-3 rounded-full" style={{ background: rgbCss(c) }} />
                ))}
                {Object.keys(p.keys).length > 0 && (
                  <span className="ml-1 text-[11px] text-muted">{t("prof.keysCount", { n: Object.keys(p.keys).length })}</span>
                )}
              </div>

              <div className="mt-auto flex gap-2">
                <button type="button" onClick={() => apply(p.id)} className="flex items-center gap-1.5 rounded-lg bg-accent px-3 py-1.5 text-xs font-semibold text-white hover:brightness-110">
                  <Play size={12} /> {t("prof.apply")}
                </button>
                <button type="button" onClick={() => onEdit(p)} className="flex items-center gap-1.5 rounded-lg border border-line px-3 py-1.5 text-xs text-muted hover:border-accent/60 hover:text-text">
                  <Pencil size={12} /> {t("prof.edit")}
                </button>
                <button
                  type="button"
                  onClick={() => (confirm === p.id ? (remove(p.id), setConfirm(null)) : setConfirm(p.id))}
                  onBlur={() => setConfirm(null)}
                  className={`ml-auto flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-xs transition ${
                    confirm === p.id ? "border-rose-400 bg-rose-500/20 text-rose-200" : "border-line text-muted hover:border-rose-400/60 hover:text-rose-300"
                  }`}
                >
                  <Trash2 size={12} /> {confirm === p.id ? t("prof.deleteConfirm", { name: p.name }) : t("prof.delete")}
                </button>
              </div>
            </li>
          );
        })}
      </ul>

      <p className="flex gap-2 rounded-xl bg-surface px-4 py-3 text-[11px] leading-relaxed text-muted">
        <Info size={14} className="mt-0.5 shrink-0" /> {t("prof.autoSwitch.hint")} {t("prof.sdkNote")}
      </p>
    </div>
  );
}
