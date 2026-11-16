import { MousePointerClick } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { ContextMenuProvider, useContextMenu } from "./components/ContextMenu";
import { DatabasePage } from "./components/DatabasePage";
import { DeviceList } from "./components/DeviceList";
import { DevicePanel } from "./components/DevicePanel";
import { Header, type View } from "./components/Header";
import { ProfileEditor } from "./components/ProfileEditor";
import { ProfilesPage } from "./components/ProfilesPage";
import { SettingsDialog } from "./components/SettingsDialog";
import { ActiveProfileWatcher } from "./components/ActiveProfileWatcher";
import { ActivityLog, SystemCard } from "./components/SidePanels";
import { newProfileId, useProfiles } from "./hooks/useProfiles";
import { useRgb } from "./hooks/useRgb";
import { useWindowControls } from "./hooks/useWindowControls";
import { I18nProvider, useI18n } from "./i18n";
import { getBackend } from "./lib/backend";
import type { Template } from "./lib/templates";
import type { Profile, Rgb } from "./lib/types";

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
  const ctx = useContextMenu();
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [view, setView] = useState<View>("devices");
  const [editing, setEditing] = useState<Profile | null>(null);
  const profiles = useProfiles();
  const windowControls = useWindowControls();
  const allIds = devices.map((d) => d.id);
  const desktop = state.backendKind === "tauri";

  const keyboard = useMemo(() => devices.find((d) => d.layout), [devices]);
  const keyboardKeys = keyboard ? keyboard.keys : null;

  // Localized tray tooltip (device count + batteries).
  useEffect(() => {
    if (!desktop) return;
    const lines = [`RGB Visual Enhancer · ${t("tray.tooltip", { n: devices.length })}`];
    for (const d of devices) if (d.battery) lines.push(`${d.name} ${d.battery.percent} %`);
    getBackend().then((b) => b.setTrayTooltip(lines.join("\n")).catch(() => {}));
  }, [desktop, devices, t]);

  const saveFromKeyboard = (keys: Record<string, Rgb>, tpl?: Template) =>
    setEditing({
      id: newProfileId(),
      name: tpl ? t(tpl.label) : "",
      games: tpl?.games ?? [],
      color: null,
      keys,
    });

  return (
    <div className="flex h-full flex-col">
      <Header
        view={view}
        onView={setView}
        scanning={state.scanning}
        onScan={() => actions.scan()}
        syncAll={state.syncAll}
        onSyncAll={actions.setSyncAll}
        onSettings={() => setSettingsOpen(true)}
        demo={state.backendKind === "demo"}
        window={windowControls}
      />

      {view === "devices" && (
        <main className="grid min-h-0 flex-1 grid-cols-1 md:grid-cols-[270px_1fr] 2xl:grid-cols-[270px_1fr_340px]">
          <aside className="hidden min-h-0 border-e border-line bg-surface/40 md:flex md:flex-col">
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
                onKeys={(keys) => getBackend().then((b) => b.setKeyColors(selected.id, keys))}
                onSaveProfile={saveFromKeyboard}
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

          <aside className="hidden min-h-0 flex-col gap-4 border-s border-line p-4 2xl:flex">
            <SystemCard system={state.system} />
            <ActivityLog log={state.log} onClear={actions.clearLog} />
          </aside>
        </main>
      )}

      {view === "profiles" && (
        <main className="min-h-0 flex-1 overflow-y-auto p-6">
          <ProfilesPage profiles={profiles} onEdit={setEditing} keyboardKeys={keyboardKeys} />
        </main>
      )}

      {view === "database" && (
        <main className="min-h-0 flex-1 overflow-y-auto p-6">
          <DatabasePage devices={devices} />
        </main>
      )}

      <ProfileEditor
        initial={editing}
        currentKeys={keyboardKeys}
        onClose={() => setEditing(null)}
        onSave={async (p) => {
          await profiles.save(p);
          setEditing(null);
          ctx.notify(t("prof.saved"));
        }}
      />
      <ActiveProfileWatcher profiles={profiles.view} />

      <SettingsDialog
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
        language={state.settings.language}
        onLanguage={actions.setLanguage}
        virtualDevices={state.settings.virtual_devices}
        onVirtualDevices={actions.setVirtualDevices}
        autostart={state.autostart}
        onAutostart={actions.setAutostart}
        logFile={state.settings.log_file ?? null}
        desktop={desktop}
      />
    </div>
  );
}
