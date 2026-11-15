import { MousePointerClick } from "lucide-react";
import { useState } from "react";
import { ContextMenuProvider } from "./components/ContextMenu";
import { DeviceList } from "./components/DeviceList";
import { DevicePanel } from "./components/DevicePanel";
import { Header } from "./components/Header";
import { SettingsDialog } from "./components/SettingsDialog";
import { ActivityLog, SystemCard } from "./components/SidePanels";
import { useRgb } from "./hooks/useRgb";
import { I18nProvider, useI18n } from "./i18n";

export default function App() {
  const rgb = useRgb();
  return (
    <I18nProvider lang={rgb.state.settings.language}>
      <ContextMenuProvider>
        <Main {...rgb} />
      </ContextMenuProvider>
    </I18nProvider>
  );
}

function Main({ state, devices, selected, actions }: ReturnType<typeof useRgb>) {
  const { t } = useI18n();
  const [settingsOpen, setSettingsOpen] = useState(false);
  const allIds = devices.map((d) => d.id);
  const desktop = state.backendKind === "tauri";

  return (
    <div className="flex h-full flex-col">
      <Header
        scanning={state.scanning}
        onScan={() => actions.scan()}
        syncAll={state.syncAll}
        onSyncAll={actions.setSyncAll}
        onSettings={() => setSettingsOpen(true)}
        demo={state.backendKind === "demo"}
      />

      <main className="grid min-h-0 flex-1 grid-cols-1 md:grid-cols-[270px_1fr] 2xl:grid-cols-[270px_1fr_340px]">
        <aside className="hidden min-h-0 border-r border-line bg-surface/40 md:flex md:flex-col">
          <DeviceList
            devices={devices}
            selectedId={state.selectedId}
            onSelect={actions.select}
            scanning={state.scanning}
            onColor={(id, c) => actions.setColor(id, c, false, allIds)}
            onEffect={(id, e, s) => actions.setEffect(id, e, s)}
          />
        </aside>

        <section className="min-h-0 overflow-y-auto p-5">
          {/* Compact device switcher for narrow windows. */}
          <select
            className="mb-4 w-full rounded-lg border border-line bg-surface px-3 py-2 text-sm md:hidden"
            value={state.selectedId ?? ""}
            onChange={(e) => actions.select(e.target.value)}
          >
            {devices.map((d) => (
              <option key={d.id} value={d.id}>
                {d.name}
              </option>
            ))}
          </select>

          {selected ? (
            <DevicePanel
              device={selected}
              syncAll={state.syncAll}
              syncCount={devices.length}
              onColor={(c) => actions.setColor(selected.id, c, state.syncAll, allIds)}
              onApplyAll={(c) => actions.setColor(selected.id, c, true, allIds)}
              onZoneColor={(z, c) => actions.setZoneColor(selected.id, z, c)}
              onEffect={(e, s) => actions.setEffect(selected.id, e, s)}
            />
          ) : (
            <div className="flex h-full flex-col items-center justify-center gap-3 text-center text-muted">
              <MousePointerClick size={32} strokeWidth={1.5} />
              <p className="text-sm">{state.scanning ? t("main.scanning") : t("main.select")}</p>
            </div>
          )}

          <div className="mt-4 grid gap-4 lg:grid-cols-2 2xl:hidden">
            <SystemCard system={state.system} />
            <div className="flex max-h-80 flex-col">
              <ActivityLog log={state.log} onClear={actions.clearLog} />
            </div>
          </div>
        </section>

        <aside className="hidden min-h-0 flex-col gap-4 border-l border-line p-4 2xl:flex">
          <SystemCard system={state.system} />
          <ActivityLog log={state.log} onClear={actions.clearLog} />
        </aside>
      </main>

      <SettingsDialog
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
        language={state.settings.language}
        onLanguage={actions.setLanguage}
        virtualDevices={state.settings.virtual_devices}
        onVirtualDevices={actions.setVirtualDevices}
        autostart={state.autostart}
        onAutostart={actions.setAutostart}
        desktop={desktop}
      />
    </div>
  );
}
