import { RefreshCw, Settings } from "lucide-react";
import { useI18n } from "../i18n";
import { Toggle } from "./ui";

export function Header(props: {
  scanning: boolean;
  onScan: () => void;
  syncAll: boolean;
  onSyncAll: (v: boolean) => void;
  onSettings: () => void;
  demo: boolean;
}) {
  const { t } = useI18n();
  return (
    <header className="flex flex-wrap items-center gap-4 border-b border-line bg-surface/60 px-5 py-3 backdrop-blur">
      <div className="flex items-center gap-3">
        <div className="rgb-ring rgb-spin h-8 w-8 rounded-xl p-[2px]">
          <div className="h-full w-full rounded-[10px] bg-bg" />
        </div>
        <div>
          <h1 className="text-sm font-bold tracking-wide">RGB Visual Enhancer</h1>
          <p className="text-[11px] text-muted">
            {t("app.subtitle")}
            {props.demo && ` · ${t("app.demo")}`}
          </p>
        </div>
      </div>

      <div className="ml-auto flex flex-wrap items-center gap-4">
        <Toggle label={t("header.sync")} checked={props.syncAll} onChange={props.onSyncAll} />
        <button
          type="button"
          onClick={props.onScan}
          disabled={props.scanning}
          className="flex items-center gap-2 rounded-lg bg-accent px-3.5 py-2 text-xs font-semibold text-white shadow-lg shadow-accent/20 transition hover:brightness-110 disabled:cursor-wait disabled:opacity-60"
        >
          <RefreshCw size={14} className={props.scanning ? "animate-spin" : ""} />
          {props.scanning ? t("header.scanning") : t("header.rescan")}
        </button>
        <button
          type="button"
          onClick={props.onSettings}
          title={t("header.settings")}
          aria-label={t("header.settings")}
          className="flex h-9 w-9 items-center justify-center rounded-lg border border-line text-muted transition hover:border-accent/60 hover:text-text"
        >
          <Settings size={16} />
        </button>
      </div>
    </header>
  );
}
