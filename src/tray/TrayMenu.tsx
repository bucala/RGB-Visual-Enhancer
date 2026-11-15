import { ChevronDown, ExternalLink, Power, RefreshCw, X } from "lucide-react";
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { BatteryBadge } from "../components/Battery";
import { DeviceIcon } from "../components/DeviceIcon";
import { Toggle } from "../components/ui";
import { I18nProvider, useI18n } from "../i18n";
import { errorMessage, getBackend, type Backend } from "../lib/backend";
import { PRESETS, rgbCss, rgbToHex } from "../lib/color";
import { connectionLabel, effectKey } from "../lib/deviceText";
import type { Device, Lang, ManagerEvent, Rgb } from "../lib/types";

/** Margin around the card so its shadow isn't clipped by the window. */
const SHADOW_PAD = 10;

/**
 * App-styled tray menu, rendered in a frameless transparent window that the
 * Rust side positions next to the tray icon (Windows/macOS).
 */
export default function TrayApp() {
  const [lang, setLang] = useState<Lang>("sk");
  const [backend, setBackend] = useState<Backend | null>(null);

  useEffect(() => {
    let off: (() => void) | undefined;
    getBackend().then(async (b) => {
      setBackend(b);
      setLang((await b.getSettings()).language);
      off = await b.onSettings((s) => setLang(s.language));
    });
    return () => off?.();
  }, []);

  return <I18nProvider lang={lang}>{backend && <TrayMenu backend={backend} />}</I18nProvider>;
}

function TrayMenu({ backend }: { backend: Backend }) {
  const { t } = useI18n();
  const [devices, setDevices] = useState<Record<string, Device>>({});
  const [expanded, setExpanded] = useState<string | null>(null);
  const [scanning, setScanning] = useState(false);
  const [autostart, setAutostart] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const root = useRef<HTMLDivElement>(null);

  const reload = useCallback(async () => {
    const [list, auto, busy] = await Promise.all([backend.listDevices(), backend.getAutostart(), backend.isScanning()]);
    setDevices(Object.fromEntries(list.map((d) => [d.id, d])));
    setAutostart(auto);
    setScanning(busy);
  }, [backend]);

  useEffect(() => {
    const offs: (() => void)[] = [];
    const onEvent = (e: ManagerEvent) => {
      if (e.type === "device_added" || e.type === "device_updated") setDevices((d) => ({ ...d, [e.device.id]: e.device }));
      else if (e.type === "device_removed")
        setDevices((d) => {
          const next = { ...d };
          delete next[e.id];
          return next;
        });
      else if (e.type === "scan_started") setScanning(true);
      else if (e.type === "scan_finished") setScanning(false);
    };
    (async () => {
      offs.push(await backend.onEvent(onEvent));
      offs.push(await backend.onResync(reload));
      offs.push(
        await backend.onTrayOpened(() => {
          setExpanded(null);
          setError(null);
          void reload();
        }),
      );
      await reload();
      // Browser demo: populate the simulator so the menu has content.
      if (backend.kind === "demo" && (await backend.listDevices()).length === 0) await backend.scan().catch(() => {});
    })();
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && backend.hideTrayMenu();
    const onContext = (e: MouseEvent) => e.preventDefault();
    window.addEventListener("keydown", onKey);
    window.addEventListener("contextmenu", onContext);
    return () => {
      offs.forEach((o) => o());
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("contextmenu", onContext);
    };
  }, [backend, reload]);

  // Size the native window to the content.
  useLayoutEffect(() => {
    const el = root.current;
    if (!el) return;
    const report = () => backend.resizeTrayMenu(el.offsetHeight + SHADOW_PAD * 2).catch(() => {});
    report();
    const ro = new ResizeObserver(report);
    ro.observe(el);
    return () => ro.disconnect();
  }, [backend]);

  const list = useMemo(() => Object.values(devices).sort((a, b) => a.name.localeCompare(b.name)), [devices]);
  const act = (p: Promise<unknown>) => p.catch((e) => setError(errorMessage(e)));
  const setColor = (id: string, c: Rgb) => act(backend.setColor(id, c));
  const lightsOff = (d: Device) => act(backend.setEffect(d.id, 0, d.speed));

  return (
    <div style={{ padding: SHADOW_PAD }}>
      <div
        ref={root}
        className="overflow-hidden rounded-2xl border border-line bg-surface text-text shadow-[0_12px_40px_rgba(0,0,0,.65)]"
      >
        <header className="flex items-center gap-3 border-b border-line px-4 py-3">
          <div className="rgb-ring rgb-spin h-7 w-7 rounded-lg p-[2px]">
            <div className="h-full w-full rounded-[6px] bg-bg" />
          </div>
          <div className="min-w-0 flex-1">
            <p className="text-[13px] font-bold">RGB Visual Enhancer</p>
            <p className="text-[11px] text-muted">{t("app.subtitle")}</p>
          </div>
          <button
            type="button"
            onClick={() => backend.hideTrayMenu()}
            aria-label={t("settings.close")}
            className="rounded-lg p-1 text-muted transition hover:bg-surface-2 hover:text-text"
          >
            <X size={15} />
          </button>
        </header>

        <ul className="max-h-[440px] overflow-y-auto p-1.5">
          {list.length === 0 && <li className="px-3 py-4 text-center text-xs text-muted">{scanning ? t("list.scanning") : t("tray.none")}</li>}
          {list.map((d) => {
            const open = expanded === d.id;
            const lit = d.color && d.effect !== 0;
            const effect = d.effect != null ? t(effectKey(d.effect)) : t("tray.notSet");
            const canColor = d.effects.some((e) => e.uses_color);
            const canOff = d.effects.some((e) => e.id === 0);
            return (
              <li key={d.id} className={`rounded-xl transition ${open ? "bg-surface-2" : ""}`}>
                <button
                  type="button"
                  onClick={() => setExpanded(open ? null : d.id)}
                  aria-expanded={open}
                  className="flex w-full items-center gap-2.5 rounded-xl px-2.5 py-2 text-left transition hover:bg-surface-2"
                >
                  <span className="relative flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-bg">
                    <DeviceIcon type={d.device_type} size={16} />
                    <span
                      className="absolute -right-0.5 -bottom-0.5 h-3 w-3 rounded-full border-2 border-surface"
                      style={{ background: lit && d.color ? rgbCss(d.color) : "#3a4150" }}
                    />
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-[13px] font-medium">{d.name}</span>
                    <span className="block truncate text-[11px] text-muted">
                      {effect}
                      {lit && d.color && canColor && d.effects.find((e) => e.id === d.effect)?.uses_color ? ` · ${rgbToHex(d.color).toUpperCase()}` : ""}
                    </span>
                  </span>
                  {d.battery && (
                    <span className="text-[11px]">
                      <BatteryBadge battery={d.battery} size={12} />
                    </span>
                  )}
                  {d.status.state === "error" && <span className="h-2 w-2 rounded-full bg-rose-500" title={d.status.message} />}
                  <ChevronDown size={14} className={`shrink-0 text-muted transition ${open ? "rotate-180" : ""}`} />
                </button>
                {open && (
                  <div className="flex flex-col gap-2 px-3 pt-1 pb-3">
                    <p className="text-[11px] text-muted">
                      {connectionLabel(d, t)} · {d.status.state === "error" ? t("status.error") : t("status.ready")}
                    </p>
                    {d.status.state === "error" && <p className="text-[11px] text-rose-300">{d.status.message}</p>}
                    <div className="flex flex-wrap items-center gap-1.5">
                      {canColor &&
                        PRESETS.map((c) => (
                          <button
                            key={rgbToHex(c)}
                            type="button"
                            title={rgbToHex(c).toUpperCase()}
                            onClick={() => setColor(d.id, c)}
                            className="h-6 w-6 rounded-full border border-white/10 transition hover:scale-115"
                            style={{ background: rgbCss(c) }}
                          />
                        ))}
                      {canOff && (
                        <button
                          type="button"
                          onClick={() => lightsOff(d)}
                          title={t("ctx.lightsOff")}
                          className="ml-auto flex h-6 items-center gap-1 rounded-full border border-line px-2 text-[11px] text-muted transition hover:border-rose-400/60 hover:text-rose-300"
                        >
                          <Power size={11} />
                          {t("ctx.lightsOff")}
                        </button>
                      )}
                    </div>
                  </div>
                )}
              </li>
            );
          })}
        </ul>

        {list.length > 0 && (
          <div className="border-t border-line px-4 py-3">
            <p className="mb-2 text-[10px] font-semibold tracking-[0.14em] text-muted uppercase">{t("tray.all")}</p>
            <div className="flex flex-wrap items-center gap-1.5">
              {PRESETS.map((c) => (
                <button
                  key={rgbToHex(c)}
                  type="button"
                  title={rgbToHex(c).toUpperCase()}
                  onClick={() => act(backend.setColorMany(list.map((d) => d.id), c))}
                  className="h-6 w-6 rounded-full border border-white/10 transition hover:scale-115"
                  style={{ background: rgbCss(c) }}
                />
              ))}
              <button
                type="button"
                onClick={() => list.forEach((d) => d.effects.some((e) => e.id === 0) && lightsOff(d))}
                title={t("ctx.lightsOff")}
                className="ml-auto flex h-6 items-center gap-1 rounded-full border border-line px-2 text-[11px] text-muted transition hover:border-rose-400/60 hover:text-rose-300"
              >
                <Power size={11} />
                {t("ctx.lightsOff")}
              </button>
            </div>
          </div>
        )}

        {error && <p className="border-t border-line px-4 py-2 text-[11px] text-rose-300">{error}</p>}

        <footer className="flex flex-col gap-2 border-t border-line p-3">
          <div className="flex gap-2">
            <button
              type="button"
              onClick={() => backend.showMainWindow()}
              className="flex flex-1 items-center justify-center gap-2 rounded-lg bg-accent px-3 py-2 text-xs font-semibold text-white transition hover:brightness-110"
            >
              <ExternalLink size={13} />
              {t("tray.open")}
            </button>
            <button
              type="button"
              onClick={() => act(backend.scan())}
              disabled={scanning}
              title={t("tray.rescan")}
              aria-label={t("tray.rescan")}
              className="flex w-9 items-center justify-center rounded-lg border border-line text-muted transition hover:border-accent/60 hover:text-text disabled:opacity-50"
            >
              <RefreshCw size={14} className={scanning ? "animate-spin" : ""} />
            </button>
          </div>
          <div className="flex items-center justify-between px-1">
            <Toggle
              label={t("tray.autostart")}
              checked={autostart}
              onChange={async (on) => {
                await act(backend.setAutostart(on));
                setAutostart(await backend.getAutostart());
              }}
            />
            <button
              type="button"
              onClick={() => backend.quit()}
              className="rounded-lg px-2 py-1 text-xs text-muted transition hover:bg-rose-500/15 hover:text-rose-300"
            >
              {t("tray.quit")}
            </button>
          </div>
        </footer>
      </div>
    </div>
  );
}
