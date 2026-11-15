import { Gauge } from "lucide-react";
import { useEffect, useState } from "react";
import type { Device } from "../lib/types";

export function EffectPanel({ device, onEffect }: { device: Device; onEffect: (effect: number, speed: number) => void }) {
  const [speed, setSpeed] = useState(device.speed);
  useEffect(() => setSpeed(device.speed), [device.id, device.speed]);

  const active = device.effects.find((e) => e.id === device.effect);

  return (
    <div className="flex flex-col gap-4">
      <div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
        {device.effects.map((e) => {
          const on = e.id === device.effect;
          return (
            <button
              key={e.id}
              type="button"
              onClick={() => onEffect(e.id, speed)}
              className={`rounded-xl border px-3 py-2.5 text-left text-xs font-medium transition ${
                on ? "border-accent bg-accent/15 text-text" : "border-line bg-bg text-muted hover:border-accent/50 hover:text-text"
              }`}
            >
              {e.name}
              <span className="mt-0.5 block text-[10px] font-normal text-muted">
                {e.uses_color ? "Uses colour" : e.uses_speed ? "Animated" : e.id === 0 ? "Lights off" : ""}
              </span>
            </button>
          );
        })}
      </div>

      <label className="flex items-center gap-3 text-xs text-muted">
        <Gauge size={14} />
        Speed
        <input
          type="range"
          min={0}
          max={100}
          value={speed}
          disabled={!active?.uses_speed}
          onChange={(e) => {
            const s = Number(e.target.value);
            setSpeed(s);
            if (active?.uses_speed) onEffect(active.id, s);
          }}
          className="slider flex-1"
        />
        <span className="w-8 text-right font-mono text-text">{speed}</span>
      </label>
    </div>
  );
}
