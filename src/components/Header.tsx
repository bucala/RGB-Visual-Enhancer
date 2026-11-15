import { RefreshCw } from "lucide-react";
import { Toggle } from "./ui";

export function Header(props: {
  scanning: boolean;
  onScan: () => void;
  syncAll: boolean;
  onSyncAll: (v: boolean) => void;
  virtualDevices: boolean;
  onVirtualDevices: (v: boolean) => void;
  demo: boolean;
}) {
  return (
    <header className="flex flex-wrap items-center gap-4 border-b border-line bg-surface/60 px-5 py-3 backdrop-blur">
      <div className="flex items-center gap-3">
        <div className="rgb-ring h-8 w-8 rounded-xl p-[2px]">
          <div className="h-full w-full rounded-[10px] bg-bg" />
        </div>
        <div>
          <h1 className="text-sm font-bold tracking-wide">RGB Visual Enhancer</h1>
          <p className="text-[11px] text-muted">Unified lighting control{props.demo && " · browser demo"}</p>
        </div>
      </div>

      <div className="ml-auto flex flex-wrap items-center gap-5">
        <Toggle label="Sync all devices" checked={props.syncAll} onChange={props.onSyncAll} />
        <Toggle label="Virtual devices" checked={props.virtualDevices} onChange={props.onVirtualDevices} />
        <button
          type="button"
          onClick={props.onScan}
          disabled={props.scanning}
          className="flex items-center gap-2 rounded-lg bg-accent px-3.5 py-2 text-xs font-semibold text-white shadow-lg shadow-accent/20 transition hover:brightness-110 disabled:cursor-wait disabled:opacity-60"
        >
          <RefreshCw size={14} className={props.scanning ? "animate-spin" : ""} />
          {props.scanning ? "Scanning…" : "Rescan hardware"}
        </button>
      </div>
    </header>
  );
}
