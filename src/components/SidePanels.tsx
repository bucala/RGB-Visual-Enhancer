import { Activity, CircleAlert, ClipboardCopy, Info, Monitor, Trash2, TriangleAlert } from "lucide-react";
import type { LogEntry } from "../hooks/useRgb";
import { useI18n, type Key, type TFn } from "../i18n";
import type { SystemInfo } from "../lib/types";
import { copyText, useContextMenu } from "./ContextMenu";
import { Card } from "./ui";

export function SystemCard({ system }: { system: SystemInfo | null }) {
  const { t } = useI18n();
  const ctx = useContextMenu();
  const rows: [string, string | null | undefined][] = [
    [t("hw.motherboard"), system && [system.board_vendor, system.board_name].filter(Boolean).join(" ")],
    [t("hw.system"), system?.system_product],
    [t("hw.cpu"), system?.cpu],
    [t("hw.bios"), system && [system.bios_vendor, system.bios_version].filter(Boolean).join(" ")],
    [t("hw.os"), system && `${system.os} (${system.arch})`],
  ];
  const text = rows.map(([k, v]) => `${k}: ${v || "—"}`).join("\n");
  return (
    <div
      onContextMenu={(e) =>
        ctx.open(e, [
          {
            kind: "item",
            label: t("ctx.copyHw"),
            icon: <ClipboardCopy size={13} />,
            onSelect: async () => {
              if (await copyText(text)) ctx.notify(t("ctx.copied"));
            },
          },
        ])
      }
    >
      <Card title={t("hw.title")} icon={<Monitor size={15} />}>
        <dl className="grid grid-cols-[108px_1fr] gap-x-3 gap-y-2 text-xs">
          {rows.map(([k, v]) => (
            <div key={k} className="contents">
              <dt className="text-muted">{k}</dt>
              <dd className="truncate" title={v ?? undefined}>
                {v || <span className="text-muted">—</span>}
              </dd>
            </div>
          ))}
        </dl>
      </Card>
    </div>
  );
}

const LEVEL = {
  info: { icon: Info, cls: "text-sky-300" },
  warn: { icon: TriangleAlert, cls: "text-amber-300" },
  error: { icon: CircleAlert, cls: "text-rose-300" },
};

const KNOWN_SOURCES = new Set(["Scan", "Settings", "Sync", "System", "Backend"]);

export function sourceText(e: LogEntry, t: TFn): string {
  return KNOWN_SOURCES.has(e.source) ? t(`src.${e.source}` as Key) : e.source;
}

export function logText(e: LogEntry, t: TFn): string {
  return typeof e.message === "string" ? e.message : t(e.message.key, e.message.vars);
}

export function ActivityLog({ log, onClear }: { log: LogEntry[]; onClear: () => void }) {
  const { t, lang } = useI18n();
  const ctx = useContextMenu();
  const line = (e: LogEntry) => `[${new Date(e.time).toLocaleTimeString(lang)}] ${sourceText(e, t)}: ${logText(e, t)}`;
  const copy = async (text: string) => {
    if (await copyText(text)) ctx.notify(t("ctx.copied"));
  };

  return (
    <Card title={t("log.title")} icon={<Activity size={15} />} className="flex min-h-0 flex-1 flex-col">
      <ol className="-m-1 flex max-h-full flex-col gap-1 overflow-y-auto p-1 text-xs">
        {log.length === 0 && <li className="text-muted">{t("log.empty")}</li>}
        {log.map((e) => {
          const { icon: Icon, cls } = LEVEL[e.level];
          return (
            <li
              key={e.id}
              className="flex gap-2 rounded-lg px-1.5 py-1 hover:bg-surface-2"
              onContextMenu={(ev) =>
                ctx.open(ev, [
                  { kind: "item", label: t("ctx.copy"), icon: <ClipboardCopy size={13} />, onSelect: () => copy(line(e)) },
                  {
                    kind: "item",
                    label: t("ctx.copyAll"),
                    icon: <ClipboardCopy size={13} />,
                    onSelect: () => copy([...log].reverse().map(line).join("\n")),
                  },
                  { kind: "separator" },
                  { kind: "item", label: t("ctx.clear"), icon: <Trash2 size={13} />, danger: true, onSelect: onClear },
                ])
              }
            >
              <Icon size={13} className={`mt-0.5 shrink-0 ${cls}`} />
              <div className="min-w-0">
                <div className="flex gap-2 text-[10px] text-muted">
                  <span className="font-medium">{sourceText(e, t)}</span>
                  <span>{new Date(e.time).toLocaleTimeString(lang)}</span>
                </div>
                <p className="break-words text-text/90">{logText(e, t)}</p>
              </div>
            </li>
          );
        })}
      </ol>
    </Card>
  );
}
