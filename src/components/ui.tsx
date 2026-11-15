import type { ReactNode } from "react";

export function Card({ title, icon, actions, children, className = "" }: {
  title: string;
  icon?: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={`rounded-2xl border border-line bg-surface ${className}`}>
      <header className="flex items-center gap-2 border-b border-line px-4 py-3">
        {icon && <span className="text-muted">{icon}</span>}
        <h2 className="text-sm font-semibold tracking-wide text-text">{title}</h2>
        <div className="ml-auto flex items-center gap-2">{actions}</div>
      </header>
      <div className="p-4">{children}</div>
    </section>
  );
}

export function Toggle({ checked, onChange, label, hideLabel }: {
  checked: boolean;
  onChange: (v: boolean) => void;
  /** Accessible name; also shown next to the switch unless `hideLabel`. */
  label: string;
  hideLabel?: boolean;
}) {
  return (
    <label className="flex cursor-pointer items-center gap-2 text-xs text-muted hover:text-text">
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        aria-label={label}
        onClick={() => onChange(!checked)}
        className={`relative h-5 w-9 rounded-full transition-colors ${checked ? "bg-accent" : "bg-line"}`}
      >
        <span
          className={`absolute top-0.5 left-0.5 h-4 w-4 rounded-full bg-white transition-transform ${checked ? "translate-x-4" : ""}`}
        />
      </button>
      {!hideLabel && label}
    </label>
  );
}

export function Chip({ children, tone = "default" }: { children: ReactNode; tone?: "default" | "ok" | "warn" | "error" }) {
  const tones = {
    default: "border-line text-muted",
    ok: "border-emerald-500/30 text-emerald-300",
    warn: "border-amber-500/30 text-amber-300",
    error: "border-rose-500/40 text-rose-300",
  };
  return (
    <span className={`inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[11px] font-medium ${tones[tone]}`}>
      {children}
    </span>
  );
}
