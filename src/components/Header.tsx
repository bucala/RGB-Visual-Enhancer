import { Copy, Database, Gamepad2, Layers, Minus, RefreshCw, Settings, Square, X, type LucideIcon } from "lucide-react";
import { useEffect, useState } from "react";
import { useI18n } from "../i18n";
import type { WindowControls } from "../lib/backend";
import { Toggle } from "./ui";

/**
 * App header doubling as the window title bar (the window is frameless):
 * empty areas drag the window, double-click maximises, and the window
 * buttons on the right replace the system ones.
 */
export type View = "devices" | "profiles" | "database";

const TABS: [View, "nav.devices" | "nav.profiles" | "nav.database", LucideIcon][] = [
  ["devices", "nav.devices", Layers],
  ["profiles", "nav.profiles", Gamepad2],
  ["database", "nav.database", Database],
];

export function Header(props: {
  view: View;
  onView: (v: View) => void;
  scanning: boolean;
  onScan: () => void;
  syncAll: boolean;
  onSyncAll: (v: boolean) => void;
  onSettings: () => void;
  demo: boolean;
  window: WindowControls | null;
}) {
  const { t } = useI18n();
  return (
    <header
      data-tauri-drag-region
      className="flex h-14 shrink-0 items-center gap-3 border-b border-line bg-surface/60 pl-4 backdrop-blur select-none"
    >
      <div data-tauri-drag-region className="flex min-w-0 items-center gap-3">
        <div className="rgb-ring rgb-spin pointer-events-none h-8 w-8 shrink-0 rounded-xl p-[2px]">
          <div className="h-full w-full rounded-[10px] bg-bg" />
        </div>
        <div className="pointer-events-none min-w-0">
          <h1 className="truncate text-sm font-bold tracking-wide">RGB Visual Enhancer</h1>
          <p className="truncate text-[11px] text-muted">
            {t("app.subtitle")}
            {props.demo && ` · ${t("app.demo")}`}
          </p>
        </div>
      </div>

      <nav data-tauri-drag-region className="ml-4 flex items-center gap-1 rounded-xl border border-line bg-bg/60 p-1" aria-label="Views">
        {TABS.map(([id, label, Icon]) => (
          <button
            key={id}
            type="button"
            onClick={() => props.onView(id)}
            aria-current={props.view === id ? "page" : undefined}
            className={`flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-medium transition ${
              props.view === id ? "bg-surface-2 text-text shadow" : "text-muted hover:text-text"
            }`}
          >
            <Icon size={13} />
            <span className="hidden lg:inline">{t(label)}</span>
          </button>
        ))}
      </nav>

      {/* Draggable spacer. */}
      <div data-tauri-drag-region className="h-full flex-1" />

      <div className="flex items-center gap-3">
        <Toggle label={t("header.sync")} checked={props.syncAll} onChange={props.onSyncAll} />
        <button
          type="button"
          onClick={props.onScan}
          disabled={props.scanning}
          className="flex items-center gap-2 rounded-lg bg-accent px-3.5 py-2 text-xs font-semibold whitespace-nowrap text-white shadow-lg shadow-accent/20 transition hover:brightness-110 disabled:cursor-wait disabled:opacity-60"
        >
          <RefreshCw size={14} className={props.scanning ? "animate-spin" : ""} />
          <span className="hidden sm:inline">{props.scanning ? t("header.scanning") : t("header.rescan")}</span>
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

      {props.window ? <WindowButtons controls={props.window} /> : <div className="w-1" />}
    </header>
  );
}

function WindowButtons({ controls }: { controls: WindowControls }) {
  const { t } = useI18n();
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    let off: (() => void) | undefined;
    const sync = () => controls.isMaximized().then(setMaximized, () => {});
    sync();
    controls.onResized(sync).then((u) => (off = u));
    return () => off?.();
  }, [controls]);

  const base = "flex h-14 w-12 items-center justify-center text-muted transition";
  return (
    <div className="ml-1 flex self-stretch border-l border-line">
      <button type="button" onClick={() => controls.minimize()} title={t("win.minimize")} aria-label={t("win.minimize")} className={`${base} hover:bg-surface-2 hover:text-text`}>
        <Minus size={16} />
      </button>
      <button
        type="button"
        onClick={() => controls.toggleMaximize()}
        title={maximized ? t("win.restore") : t("win.maximize")}
        aria-label={maximized ? t("win.restore") : t("win.maximize")}
        className={`${base} hover:bg-surface-2 hover:text-text`}
      >
        {maximized ? <Copy size={13} className="-scale-x-100" /> : <Square size={13} />}
      </button>
      <button type="button" onClick={() => controls.close()} title={t("win.close")} aria-label={t("win.close")} className={`${base} hover:bg-rose-600 hover:text-white`}>
        <X size={17} />
      </button>
    </div>
  );
}
