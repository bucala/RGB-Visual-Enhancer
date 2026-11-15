import { Check } from "lucide-react";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type MouseEvent as RMouseEvent,
  type ReactNode,
} from "react";
import { rgbCss, rgbToHex } from "../lib/color";
import type { Rgb } from "../lib/types";

/**
 * Desktop-style context menus.
 *
 * The webview's own menu (Back, Reload, Print, Inspect…) is suppressed
 * everywhere except in text fields, where the native Cut/Copy/Paste menu is
 * the right tool. Components that have something useful to offer open an
 * app-styled menu via `useContextMenu()`.
 */
export type CtxItem =
  | { kind: "header"; label: string }
  | { kind: "separator" }
  | { kind: "swatches"; colors: Rgb[]; onPick: (c: Rgb) => void }
  | {
      kind: "item";
      label: string;
      icon?: ReactNode;
      onSelect: () => void;
      checked?: boolean;
      danger?: boolean;
      disabled?: boolean;
    };

interface Api {
  open: (e: RMouseEvent, items: CtxItem[]) => void;
  notify: (text: string) => void;
}

const Ctx = createContext<Api>({ open: () => {}, notify: () => {} });
export const useContextMenu = () => useContext(Ctx);

function isEditable(t: EventTarget | null): boolean {
  if (!(t instanceof HTMLElement)) return false;
  if (t.isContentEditable) return true;
  if (t instanceof HTMLTextAreaElement) return true;
  if (t instanceof HTMLInputElement) return !["checkbox", "radio", "range", "button", "submit", "color"].includes(t.type);
  return false;
}

/** Browser shortcuts that make no sense in a desktop app. */
function isBrowserShortcut(e: KeyboardEvent): boolean {
  const k = e.key.toLowerCase();
  if (["f3", "f5", "f7", "browserback", "browserforward", "browserrefresh"].includes(k)) return true;
  if (e.altKey && (k === "arrowleft" || k === "arrowright")) return true;
  if (e.ctrlKey || e.metaKey) {
    if (["r", "p", "f", "g", "u", "s", "o", "j", "h", "n", "t", "w", "+", "-", "=", "0"].includes(k)) return true;
  }
  return false;
}

export function ContextMenuProvider({ children }: { children: ReactNode }) {
  const [menu, setMenu] = useState<{ x: number; y: number; items: CtxItem[] } | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ left: number; top: number } | null>(null);

  const close = useCallback(() => setMenu(null), []);

  const open = useCallback((e: RMouseEvent, items: CtxItem[]) => {
    e.preventDefault();
    e.stopPropagation();
    setPos(null);
    setMenu({ x: e.clientX, y: e.clientY, items });
  }, []);

  const notify = useCallback((text: string) => {
    setToast(text);
    window.setTimeout(() => setToast((t) => (t === text ? null : t)), 1600);
  }, []);

  // Suppress the webview menu and browser shortcuts globally.
  useEffect(() => {
    const onContext = (e: MouseEvent) => {
      if (!isEditable(e.target)) e.preventDefault();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setMenu(null);
      // Keep reload & co. while developing.
      // (Editing shortcuts — Ctrl+C/V/X/A/Z — are not in the list.)
      if (!import.meta.env.DEV && isBrowserShortcut(e)) e.preventDefault();
    };
    document.addEventListener("contextmenu", onContext);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("contextmenu", onContext);
      document.removeEventListener("keydown", onKey);
    };
  }, []);

  // Close on outside click, scroll, resize or window blur.
  useEffect(() => {
    if (!menu) return;
    const onDown = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) close();
    };
    window.addEventListener("mousedown", onDown);
    window.addEventListener("blur", close);
    window.addEventListener("resize", close);
    window.addEventListener("wheel", close, { passive: true });
    return () => {
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("blur", close);
      window.removeEventListener("resize", close);
      window.removeEventListener("wheel", close);
    };
  }, [menu, close]);

  // Keep the menu inside the viewport.
  useLayoutEffect(() => {
    if (!menu || !ref.current) return;
    const r = ref.current.getBoundingClientRect();
    const pad = 8;
    setPos({
      left: Math.max(pad, Math.min(menu.x, window.innerWidth - r.width - pad)),
      top: Math.max(pad, Math.min(menu.y, window.innerHeight - r.height - pad)),
    });
  }, [menu]);

  const run = (fn: () => void) => {
    close();
    fn();
  };

  return (
    <Ctx.Provider value={{ open, notify }}>
      {children}
      {menu && (
        <div
          ref={ref}
          role="menu"
          className="fixed z-50 min-w-52 max-w-80 rounded-xl border border-line bg-surface-2/95 p-1 text-[13px] shadow-2xl shadow-black/60 backdrop-blur"
          style={{ left: pos?.left ?? menu.x, top: pos?.top ?? menu.y, visibility: pos ? "visible" : "hidden" }}
          onContextMenu={(e) => e.preventDefault()}
        >
          {menu.items.map((item, i) => {
            switch (item.kind) {
              case "header":
                return (
                  <div key={i} className="truncate px-2.5 pt-1.5 pb-1 text-[10px] font-semibold tracking-[0.12em] text-muted uppercase">
                    {item.label}
                  </div>
                );
              case "separator":
                return <div key={i} className="my-1 h-px bg-line" />;
              case "swatches":
                return (
                  <div key={i} className="flex flex-wrap gap-1.5 px-2.5 py-1.5">
                    {item.colors.map((c) => (
                      <button
                        key={rgbToHex(c)}
                        type="button"
                        role="menuitem"
                        title={rgbToHex(c).toUpperCase()}
                        onClick={() => run(() => item.onPick(c))}
                        className="h-5 w-5 rounded-full border border-white/10 transition hover:scale-125 focus-visible:outline-2 focus-visible:outline-accent"
                        style={{ background: rgbCss(c) }}
                      />
                    ))}
                  </div>
                );
              case "item":
                return (
                  <button
                    key={i}
                    type="button"
                    role="menuitem"
                    disabled={item.disabled}
                    onClick={() => run(item.onSelect)}
                    className={`flex w-full items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-left transition disabled:opacity-40 ${
                      item.danger ? "text-rose-300 hover:bg-rose-500/15" : "text-text hover:bg-accent/20"
                    }`}
                  >
                    <span className="flex w-4 justify-center text-muted">
                      {item.checked ? <Check size={14} className="text-accent" /> : item.icon}
                    </span>
                    <span className="truncate">{item.label}</span>
                  </button>
                );
            }
          })}
        </div>
      )}
      {toast && (
        <div className="pointer-events-none fixed bottom-5 left-1/2 z-50 -translate-x-1/2 rounded-full border border-line bg-surface-2 px-4 py-2 text-xs text-text shadow-xl">
          {toast}
        </div>
      )}
    </Ctx.Provider>
  );
}

export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}
