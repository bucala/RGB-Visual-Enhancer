import { Eraser, Keyboard, PaintBucket, Pipette, Power, Save, Sparkles } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useI18n } from "../i18n";
import { PRESETS, rgbCss } from "../lib/color";
import { ALL_KEY_IDS, G910_BADGE, G910_GKEYS, G910_LOGO, G910_ROWS, ROW_UNITS, isKey, type KeyDef } from "../lib/keyboardLayout";
import { TEMPLATES, type Template } from "../lib/templates";
import type { Device, Rgb } from "../lib/types";
import { useContextMenu } from "./ContextMenu";
import { Card, Chip } from "./ui";

const OFF: Rgb = { r: 0, g: 0, b: 0 };
const LEFT = 1.4; // G-key column + gap, in key units
const TOP = 1.4; // logo / G6–G9 strip + gap
const BOTTOM = 1.25; // palm-rest badge strip below the keys
const WIDTH = LEFT + ROW_UNITS;
const HEIGHT = TOP + G910_ROWS.length + BOTTOM;

interface Placed extends KeyDef {
  x: number;
  y: number;
  w: number;
}

/** Absolute positions of every key, in key units. */
function placeKeys(): Placed[] {
  const out: Placed[] = [];
  G910_ROWS.forEach((row, r) => {
    let x = LEFT;
    for (const item of row) {
      const w = isKey(item) ? (item.w ?? 1) : item.gap;
      if (isKey(item)) out.push({ ...item, x, y: TOP + r, w });
      x += w;
    }
  });
  // G1–G5 down the left edge, G6–G9 above F1–F4, logo top-left.
  G910_GKEYS.slice(0, 5).forEach((k, i) => out.push({ ...k, x: 0, y: TOP + 1 + i, w: 1 }));
  G910_GKEYS.slice(5).forEach((k, i) => out.push({ ...k, x: LEFT + 2 + i, y: 0, w: 1 }));
  out.push({ ...G910_LOGO, x: 0, y: 0, w: 1 });
  // The "G910" badge sits on the palm rest, below Ctrl/Win/Alt.
  out.push({ ...G910_BADGE, x: LEFT + 1.75, y: TOP + G910_ROWS.length + 0.25, w: 2.25 });
  return out;
}

const luminance = (c: Rgb) => 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b;

export function KeyboardEditor({ device, brush, onBrush, onSend, onSaveProfile }: {
  device: Device;
  /** Current colour from the colour picker. */
  brush: Rgb;
  onBrush: (c: Rgb) => void;
  onSend: (keys: [string, Rgb][]) => Promise<unknown>;
  onSaveProfile: (keys: Record<string, Rgb>, template?: Template) => void;
}) {
  const { t } = useI18n();
  const ctx = useContextMenu();
  const keys = useMemo(placeKeys, []);
  const [colors, setColors] = useState<Record<string, Rgb>>(device.keys);
  const [template, setTemplate] = useState<Template | undefined>();
  const painting = useRef(false);

  // Follow the device when switching keyboards or after profile changes.
  useEffect(() => setColors(device.keys), [device.id, device.keys]);

  // Batched, latest-wins sending of painted keys.
  const pending = useRef<Record<string, Rgb>>({});
  const busy = useRef(false);
  const flush = async () => {
    if (busy.current) return;
    busy.current = true;
    try {
      while (Object.keys(pending.current).length) {
        const batch = pending.current;
        pending.current = {};
        await onSend(Object.entries(batch)).catch(() => {});
      }
    } finally {
      busy.current = false;
    }
  };
  const paint = (ids: string[], c: Rgb) => {
    setColors((prev) => {
      const next = { ...prev };
      for (const id of ids) next[id] = c;
      return next;
    });
    for (const id of ids) pending.current[id] = c;
    void flush();
  };
  const applyMap = (map: Record<string, Rgb>) => {
    setColors(map);
    Object.assign(pending.current, map);
    void flush();
  };

  useEffect(() => {
    const stop = () => (painting.current = false);
    window.addEventListener("pointerup", stop);
    return () => window.removeEventListener("pointerup", stop);
  }, []);

  return (
    <Card
      title={t("kb.title")}
      icon={<Keyboard size={15} />}
      actions={<Chip tone="warn">{t("db.support.experimental")}</Chip>}
    >
      <div className="mb-3 flex flex-wrap items-center gap-2">
        <span
          className="h-7 w-7 shrink-0 rounded-lg border border-white/20"
          style={{ background: rgbCss(brush) }}
          title={rgbCss(brush)}
        />
        <ToolButton icon={<PaintBucket size={13} />} label={t("kb.fill")} onClick={() => applyMap(Object.fromEntries(ALL_KEY_IDS.map((id) => [id, brush])))} />
        <ToolButton icon={<Eraser size={13} />} label={t("kb.clear")} onClick={() => applyMap(Object.fromEntries(ALL_KEY_IDS.map((id) => [id, OFF])))} />
        <span className="mx-1 h-5 w-px bg-line" />
        <Sparkles size={13} className="text-muted" />
        <span className="text-[11px] text-muted">{t("kb.templates")}:</span>
        {TEMPLATES.map((tpl) => (
          <button
            key={tpl.id}
            type="button"
            onClick={() => {
              setTemplate(tpl);
              applyMap(tpl.keys());
            }}
            className={`rounded-full border px-2.5 py-1 text-[11px] transition ${
              template?.id === tpl.id ? "border-accent bg-accent/15 text-text" : "border-line text-muted hover:border-accent/50 hover:text-text"
            }`}
          >
            {t(tpl.label)}
          </button>
        ))}
        <button
          type="button"
          onClick={() => onSaveProfile(colors, template)}
          className="ml-auto flex items-center gap-1.5 rounded-lg bg-accent px-3 py-1.5 text-xs font-semibold text-white transition hover:brightness-110"
        >
          <Save size={13} />
          {t("kb.saveProfile")}
        </button>
      </div>

      <div
        className="relative w-full touch-none select-none"
        style={{ aspectRatio: `${WIDTH} / ${HEIGHT}` }}
        onPointerLeave={() => (painting.current = false)}
        dir="ltr"
      >
        {keys.map((k) => {
          const c = colors[k.id];
          const lit = c && (c.r || c.g || c.b);
          return (
            <button
              key={k.id}
              type="button"
              aria-label={k.label || k.id}
              onPointerDown={(e) => {
                if (e.button !== 0) return;
                painting.current = true;
                paint([k.id], brush);
              }}
              onPointerEnter={(e) => painting.current && e.buttons & 1 && paint([k.id], brush)}
              onContextMenu={(e) =>
                ctx.open(e, [
                  { kind: "header", label: k.label || k.id },
                  { kind: "item", label: t("kb.pick"), icon: <Pipette size={13} />, disabled: !c, onSelect: () => c && onBrush(c) },
                  { kind: "item", label: t("kb.off"), icon: <Power size={13} />, onSelect: () => paint([k.id], OFF) },
                  { kind: "swatches", colors: PRESETS, onPick: (p) => paint([k.id], p) },
                ])
              }
              className="absolute flex items-center justify-center rounded-[5px] border text-[clamp(7px,0.9vw,11px)] leading-none font-medium transition-[filter] hover:brightness-125"
              style={{
                left: `${((k.x + 0.06) / WIDTH) * 100}%`,
                top: `${((k.y + 0.06) / HEIGHT) * 100}%`,
                width: `${((k.w - 0.12) / WIDTH) * 100}%`,
                height: `${(0.88 / HEIGHT) * 100}%`,
                background: lit ? rgbCss(c) : "#141821",
                borderColor: lit ? "rgba(255,255,255,.25)" : "#232937",
                color: lit && luminance(c) > 140 ? "#0b0d12" : "#c9cfdb",
                boxShadow: lit ? `0 0 10px ${rgbCss(c)}55` : undefined,
              }}
            >
              {k.label}
            </button>
          );
        })}
      </div>
      <p className="mt-3 text-[11px] text-muted">{t("kb.hint")}</p>
    </Card>
  );
}

function ToolButton({ icon, label, onClick }: { icon: React.ReactNode; label: string; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex items-center gap-1.5 rounded-lg border border-line px-2.5 py-1.5 text-[11px] text-muted transition hover:border-accent/50 hover:text-text"
    >
      {icon}
      {label}
    </button>
  );
}
