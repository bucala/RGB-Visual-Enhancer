import { MousePointerClick } from "lucide-react";
import { DeviceList } from "./components/DeviceList";
import { DevicePanel } from "./components/DevicePanel";
import { Header } from "./components/Header";
import { ActivityLog, SystemCard } from "./components/SidePanels";
import { useRgb } from "./hooks/useRgb";

export default function App() {
  const { state, devices, selected, actions } = useRgb();
  const allIds = devices.map((d) => d.id);

  return (
    <div className="flex h-full flex-col">
      <Header
        scanning={state.scanning}
        onScan={() => actions.scan()}
        syncAll={state.syncAll}
        onSyncAll={actions.setSyncAll}
        virtualDevices={state.settings.virtual_devices}
        onVirtualDevices={actions.setVirtualDevices}
        demo={state.backendKind === "demo"}
      />

      <main className="grid min-h-0 flex-1 grid-cols-1 md:grid-cols-[270px_1fr] 2xl:grid-cols-[270px_1fr_340px]">
        <aside className="hidden min-h-0 border-r border-line bg-surface/40 md:flex md:flex-col">
          <DeviceList devices={devices} selectedId={state.selectedId} onSelect={actions.select} scanning={state.scanning} />
        </aside>

        <section className="min-h-0 overflow-y-auto p-5">
          {/* Compact device switcher for narrow windows. */}
          <select
            className="mb-4 w-full rounded-lg border border-line bg-surface px-3 py-2 text-sm md:hidden"
            value={state.selectedId ?? ""}
            onChange={(e) => actions.select(e.target.value)}
          >
            {devices.map((d) => (
              <option key={d.id} value={d.id}>{d.name}</option>
            ))}
          </select>

          {selected ? (
            <DevicePanel
              device={selected}
              syncAll={state.syncAll}
              syncCount={devices.length}
              onColor={(c) => actions.setColor(selected.id, c, state.syncAll, allIds)}
              onZoneColor={(z, c) => actions.setZoneColor(selected.id, z, c)}
              onEffect={(e, s) => actions.setEffect(selected.id, e, s)}
            />
          ) : (
            <div className="flex h-full flex-col items-center justify-center gap-3 text-center text-muted">
              <MousePointerClick size={32} strokeWidth={1.5} />
              <p className="text-sm">{state.scanning ? "Scanning for devices…" : "Select a device to start."}</p>
            </div>
          )}

          <div className="mt-4 grid gap-4 2xl:hidden lg:grid-cols-2">
            <SystemCard system={state.system} />
            <div className="flex max-h-80 flex-col"><ActivityLog log={state.log} /></div>
          </div>
        </section>

        <aside className="hidden min-h-0 flex-col gap-4 border-l border-line p-4 2xl:flex">
          <SystemCard system={state.system} />
          <ActivityLog log={state.log} />
        </aside>
      </main>
    </div>
  );
}
