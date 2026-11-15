import { Languages, Monitor, Power, X } from "lucide-react";
import { useEffect, useRef } from "react";
import { useI18n } from "../i18n";
import type { Lang } from "../lib/types";
import { Toggle } from "./ui";

const LANGS: { id: Lang; label: string; flag: string }[] = [
  { id: "sk", label: "Slovenčina", flag: "SK" },
  { id: "en", label: "English", flag: "EN" },
];

export function SettingsDialog(props: {
  open: boolean;
  onClose: () => void;
  language: Lang;
  onLanguage: (l: Lang) => void;
  virtualDevices: boolean;
  onVirtualDevices: (v: boolean) => void;
  autostart: boolean;
  onAutostart: (v: boolean) => void;
  desktop: boolean;
}) {
  const { t } = useI18n();
  const ref = useRef<HTMLDialogElement>(null);

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
      className="m-auto w-[min(440px,calc(100vw-32px))] rounded-2xl border border-line bg-surface p-0 text-text shadow-2xl shadow-black/60 backdrop:bg-black/60 backdrop:backdrop-blur-sm"
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
          <div className="grid grid-cols-2 gap-2" role="radiogroup" aria-label={t("settings.language")}>
            {LANGS.map((l) => {
              const on = props.language === l.id;
              return (
                <button
                  key={l.id}
                  type="button"
                  role="radio"
                  aria-checked={on}
                  onClick={() => props.onLanguage(l.id)}
                  className={`flex items-center gap-3 rounded-xl border px-3 py-2.5 text-left text-sm transition ${
                    on ? "border-accent bg-accent/15" : "border-line bg-bg hover:border-accent/50"
                  }`}
                >
                  <span className="rounded-md bg-surface-2 px-1.5 py-0.5 font-mono text-[10px] font-bold text-muted">{l.flag}</span>
                  {l.label}
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
