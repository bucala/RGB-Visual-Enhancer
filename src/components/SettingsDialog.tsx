import { FileText, Languages, Monitor, Power, Search, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { LANG_INFO, useI18n } from "../i18n";
import { LANGS, type Lang } from "../lib/types";
import { Toggle } from "./ui";


export function SettingsDialog(props: {
  open: boolean;
  onClose: () => void;
  language: Lang;
  onLanguage: (l: Lang) => void;
  virtualDevices: boolean;
  onVirtualDevices: (v: boolean) => void;
  autostart: boolean;
  onAutostart: (v: boolean) => void;
  logFile: string | null;
  desktop: boolean;
}) {
  const { t } = useI18n();
  const ref = useRef<HTMLDialogElement>(null);
  const [query, setQuery] = useState("");
  const q = query.trim().toLowerCase();
  const langs = LANGS.filter(
    (l) => !q || l.toLowerCase().includes(q) || LANG_INFO[l].native.toLowerCase().includes(q) || LANG_INFO[l].english.toLowerCase().includes(q),
  );

  useEffect(() => {
    const d = ref.current;
    if (!d) return;
    if (props.open && !d.open) d.showModal();
    if (!props.open && d.open) d.close();
  }, [props.open]);

  return (
    <dialog
      ref={ref}
      onClose={props.onClose}
      onClick={(e) => e.target === ref.current && props.onClose()}
      className="m-auto w-[min(520px,calc(100vw-32px))] rounded-2xl border border-line bg-surface p-0 text-text shadow-2xl shadow-black/60 backdrop:bg-black/60 backdrop:backdrop-blur-sm"
    >
      <header className="flex items-center gap-2 border-b border-line px-5 py-4">
        <h2 className="text-sm font-semibold">{t("settings.title")}</h2>
        <button
          type="button"
          onClick={props.onClose}
          aria-label={t("settings.close")}
          className="ml-auto rounded-lg p-1.5 text-muted transition hover:bg-surface-2 hover:text-text"
        >
          <X size={16} />
        </button>
      </header>

      <div className="flex flex-col gap-5 p-5">
        <section>
          <h3 className="mb-2 flex items-center gap-2 text-xs font-semibold text-muted">
            <Languages size={14} /> {t("settings.language")}
          </h3>
          <label className="mb-2 flex items-center gap-2 rounded-lg border border-line bg-bg px-3 py-1.5">
            <Search size={13} className="text-muted" />
            <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t("settings.languageSearch")} className="w-full bg-transparent text-sm outline-none" />
          </label>
          <div className="grid max-h-56 grid-cols-2 gap-1.5 overflow-y-auto pe-1" role="radiogroup" aria-label={t("settings.language")}>
            {langs.map((l) => {
              const on = props.language === l;
              return (
                <button
                  key={l}
                  type="button"
                  role="radio"
                  aria-checked={on}
                  onClick={() => props.onLanguage(l)}
                  className={`flex items-center gap-2 rounded-xl border px-2.5 py-2 text-start text-sm transition ${
                    on ? "border-accent bg-accent/15" : "border-line bg-bg hover:border-accent/50"
                  }`}
                >
                  <span className="w-14 shrink-0 rounded-md bg-surface-2 px-1 py-0.5 text-center font-mono text-[10px] font-bold text-muted">{l}</span>
                  <span className="min-w-0">
                    <span className="block truncate" lang={l}>
                      {LANG_INFO[l].native}
                    </span>
                    <span className="block truncate text-[10px] text-muted">{LANG_INFO[l].english}</span>
                  </span>
                </button>
              );
            })}
          </div>
        </section>

        <section className="flex flex-col gap-3">
          <Row
            icon={<Monitor size={14} />}
            title={t("settings.virtual")}
            hint={t("settings.virtual.hint")}
            control={<Toggle label={t("settings.virtual")} hideLabel checked={props.virtualDevices} onChange={props.onVirtualDevices} />}
          />
          {props.desktop && (
            <Row
              icon={<Power size={14} />}
              title={t("settings.autostart")}
              hint={t("settings.autostart.hint")}
              control={<Toggle label={t("settings.autostart")} hideLabel checked={props.autostart} onChange={props.onAutostart} />}
            />
          )}
        </section>

        {props.desktop && <p className="rounded-xl bg-bg px-3 py-2.5 text-[11px] leading-relaxed text-muted">{t("settings.tray")}</p>}
        {props.logFile && (
          <p className="flex items-start gap-2 text-[11px] text-muted">
            <FileText size={13} className="mt-0.5 shrink-0" />
            <span className="min-w-0">
              {t("settings.logFile")}: <span className="font-mono break-all select-text">{props.logFile}</span>
            </span>
          </p>
        )}

        <p className="text-[11px] text-muted">
          RGB Visual Enhancer · {t("settings.version")} {__APP_VERSION__}
        </p>
      </div>
    </dialog>
  );
}

function Row({ icon, title, hint, control }: { icon: React.ReactNode; title: string; hint: string; control: React.ReactNode }) {
  return (
    <div className="flex items-center gap-3 rounded-xl border border-line bg-bg px-3 py-2.5">
      <span className="text-muted">{icon}</span>
      <span className="min-w-0 flex-1">
        <span className="block text-sm">{title}</span>
        <span className="block text-[11px] text-muted">{hint}</span>
      </span>
      {control}
    </div>
  );
}
