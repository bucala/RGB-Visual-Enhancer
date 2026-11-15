import { useEffect, useRef, useState, type PointerEvent as RPointerEvent } from "react";
import { hexToRgb, hsvToRgb, PRESETS, rgbCss, rgbToHex, rgbToHsv, type Hsv } from "../lib/color";
import type { Rgb } from "../lib/types";

const clamp = (n: number, lo = 0, hi = 1) => Math.min(hi, Math.max(lo, n));

/** Drag helper: calls `onMove` with 0–1 coords relative to the element. */
function useDrag(onMove: (x: number, y: number) => void) {
  const ref = useRef<HTMLDivElement>(null);
  const handle = (e: RPointerEvent<HTMLDivElement>) => {
    const rect = ref.current!.getBoundingClientRect();
    onMove(clamp((e.clientX - rect.left) / rect.width), clamp((e.clientY - rect.top) / rect.height));
  };
  return {
    ref,
    onPointerDown: (e: RPointerEvent<HTMLDivElement>) => {
      e.currentTarget.setPointerCapture(e.pointerId);
      handle(e);
    },
    onPointerMove: (e: RPointerEvent<HTMLDivElement>) => {
      if (e.currentTarget.hasPointerCapture(e.pointerId)) handle(e);
    },
  };
}

export function ColorPicker({ value, onChange, disabled }: { value: Rgb; onChange: (c: Rgb) => void; disabled?: boolean }) {
  // Keep HSV locally so hue survives at zero saturation/value.
  const [hsv, setHsv] = useState<Hsv>(() => rgbToHsv(value));
  const [hex, setHex] = useState(rgbToHex(value));
  const lastEmitted = useRef(rgbToHex(value));

  // Follow external changes (device switch, sync from hardware).
  useEffect(() => {
    const h = rgbToHex(value);
    if (h !== lastEmitted.current) {
      lastEmitted.current = h;
      setHsv(rgbToHsv(value));
      setHex(h);
    }
  }, [value]);

  const commit = (next: Hsv) => {
    setHsv(next);
    const rgb = hsvToRgb(next);
    const h = rgbToHex(rgb);
    setHex(h);
    lastEmitted.current = h;
    onChange(rgb);
  };

  const sv = useDrag((x, y) => commit({ ...hsv, s: x, v: 1 - y }));
  const hue = useDrag((x) => commit({ ...hsv, h: x * 360 }));
  const current = hsvToRgb(hsv);
  const pure = hsvToRgb({ h: hsv.h, s: 1, v: 1 });

  return (
    <div className={`flex flex-col gap-4 ${disabled ? "pointer-events-none opacity-40" : ""}`}>
      <div
        {...sv}
        role="slider"
        aria-label="Saturation and brightness"
        aria-valuenow={Math.round(hsv.s * 100)}
        className="relative h-44 w-full cursor-crosshair touch-none"
      >
        <div className="absolute inset-0 overflow-hidden rounded-xl" style={{ background: rgbCss(pure) }}>
          <div className="absolute inset-0 bg-gradient-to-r from-white to-transparent" />
          <div className="absolute inset-0 bg-gradient-to-t from-black to-transparent" />
        </div>
        <div
          className="pointer-events-none absolute h-4 w-4 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white shadow-[0_0_0_1px_rgba(0,0,0,.4)]"
          style={{ left: `${hsv.s * 100}%`, top: `${(1 - hsv.v) * 100}%`, background: rgbCss(current) }}
        />
      </div>

      <div
        {...hue}
        role="slider"
        aria-label="Hue"
        aria-valuenow={Math.round(hsv.h)}
        className="hue-track relative h-3 w-full cursor-pointer touch-none rounded-full"
      >
        <div
          className="pointer-events-none absolute top-1/2 h-5 w-5 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white shadow"
          style={{ left: `${(hsv.h / 360) * 100}%`, background: rgbCss(pure) }}
        />
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <div className="h-10 w-10 shrink-0 rounded-lg border border-line" style={{ background: rgbCss(current) }} />
        <label className="flex min-w-[8.5rem] flex-1 items-center gap-2 rounded-lg border border-line bg-bg px-3 py-2">
          <span className="text-[11px] text-muted">HEX</span>
          <input
            value={hex}
            onChange={(e) => {
              setHex(e.target.value);
              const rgb = hexToRgb(e.target.value);
              if (rgb) commit(rgbToHsv(rgb));
            }}
            spellCheck={false}
            className="w-full bg-transparent font-mono text-sm uppercase outline-none"
          />
        </label>
        <div className="flex gap-2">
        {(["r", "g", "b"] as const).map((k) => (
          <label key={k} className="flex w-[4.25rem] items-center gap-1.5 rounded-lg border border-line bg-bg px-2 py-2">
            <span className="text-[11px] text-muted uppercase">{k}</span>
            <input
              type="number"
              min={0}
              max={255}
              value={current[k]}
              onChange={(e) => {
                const n = Math.round(clamp(Number(e.target.value) || 0, 0, 255));
                commit(rgbToHsv({ ...current, [k]: n }));
              }}
              className="no-spin w-full bg-transparent font-mono text-sm outline-none"
            />
          </label>
        ))}
        </div>
      </div>

      <div className="flex flex-wrap gap-2">
        {PRESETS.map((p) => (
          <button
            key={rgbToHex(p)}
            type="button"
            aria-label={`Preset ${rgbToHex(p)}`}
            onClick={() => commit(rgbToHsv(p))}
            className="h-7 w-7 rounded-full border border-white/10 transition hover:scale-110"
            style={{ background: rgbCss(p) }}
          />
        ))}
      </div>
    </div>
  );
}
