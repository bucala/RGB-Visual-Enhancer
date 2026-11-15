import { Activity, CircleAlert, Info, Monitor, TriangleAlert } from "lucide-react";
import type { LogEntry } from "../hooks/useRgb";
import type { SystemInfo } from "../lib/types";
import { Card } from "./ui";

export function SystemCard({ system }: { system: SystemInfo | null }) {
  const rows: [string, string | null | undefined][] = [
    ["Motherboard", system && [system.board_vendor, system.board_name].filter(Boolean).join(" ")],
    ["System", system?.system_product],
    ["CPU", system?.cpu],
    ["BIOS", system && [system.bios_vendor, system.bios_version].filter(Boolean).join(" ")],
    ["OS", system && `${system.os} (${system.arch})`],
  ];
  return (
    <Card title="Hardware" icon={<Monitor size={15} />}>
      <dl className="grid grid-cols-[88px_1fr] gap-x-3 gap-y-2 text-xs">
        {rows.map(([k, v]) => (
          <div key={k} className="contents">
            <dt className="text-muted">{k}</dt>
            <dd className="truncate" title={v ?? undefined}>{v || <span className="text-muted">—</span>}</dd>
          </div>
        ))}
      </dl>
    </Card>
  );
}

const LEVEL = {
  info: { icon: Info, cls: "text-sky-300" },
  warn: { icon: TriangleAlert, cls: "text-amber-300" },
  error: { icon: CircleAlert, cls: "text-rose-300" },
};

export function ActivityLog({ log }: { log: LogEntry[] }) {
  return (
    <Card title="Activity" icon={<Activity size={15} />} className="flex min-h-0 flex-1 flex-col">
      <ol className="-m-1 flex max-h-full flex-col gap-1 overflow-y-auto p-1 text-xs">
        {log.length === 0 && <li className="text-muted">Nothing yet.</li>}
        {log.map((e) => {
          const { icon: Icon, cls } = LEVEL[e.level];
          return (
            <li key={e.id} className="flex gap-2 rounded-lg px-1.5 py-1 hover:bg-surface-2">
              <Icon size={13} className={`mt-0.5 shrink-0 ${cls}`} />
              <div className="min-w-0">
                <div className="flex gap-2 text-[10px] text-muted">
                  <span className="font-medium">{e.source}</span>
                  <span>{new Date(e.time).toLocaleTimeString()}</span>
                </div>
                <p className="break-words text-text/90">{e.message}</p>
              </div>
            </li>
          );
        })}
      </ol>
    </Card>
  );
}
