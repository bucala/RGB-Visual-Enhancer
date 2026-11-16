import { CheckCircle2, Search } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useI18n, type Key } from "../i18n";
import { getBackend } from "../lib/backend";
import type { Device, KnownDevice, Support } from "../lib/types";
import { DeviceIcon } from "./DeviceIcon";

const SUPPORT_TONE: Record<Support, string> = {
  verified: "bg-emerald-400/15 text-emerald-300 border-emerald-400/30",
  supported: "bg-sky-400/15 text-sky-300 border-sky-400/30",
  experimental: "bg-amber-400/15 text-amber-300 border-amber-400/30",
  planned: "bg-white/5 text-muted border-line",
};
const SUPPORTS: Support[] = ["verified", "supported", "experimental", "planned"];

/** The database notes come from the Rust side in English; translate the known ones. */
const NOTES: [RegExp, Key][] = [
  [/^Confirmed on hardware/, "dbnote.confirmed"],
  [/^Detected on a real PC/, "dbnote.detected"],
  [/^Linux only/, "dbnote.linuxOnly"],
  [/^Older MSI controller/, "dbnote.msiV1"],
  [/^Open protocol/, "dbnote.esp32"],
  [/^Paired devices/, "dbnote.pairedSlots"],
  [/0x8081/, "dbnote.perKey8081"],
  [/^Per-key lighting \(experimental\)/, "dbnote.perKeyExp"],
  [/PCI ids to be added/, "dbnote.pciTodo"],
  [/^Same protocol as the motherboard/, "dbnote.sameProtocolMb"],
  [/different protocol/, "dbnote.otherProtocol"],
];

export function DatabasePage({ devices }: { devices: Device[] }) {
  const { t } = useI18n();
  const [db, setDb] = useState<KnownDevice[]>([]);
  const [query, setQuery] = useState("");
  const [support, setSupport] = useState<Support | "all" | "connected">("all");
  const [vendor, setVendor] = useState("all");

  useEffect(() => {
    getBackend().then(async (b) => setDb(await b.getDeviceDatabase()));
  }, []);

  const note = (n: string) => {
    const hit = NOTES.find(([re]) => re.test(n));
    return hit ? t(hit[1]) : n;
  };
  const isConnected = (k: KnownDevice) => !!k.match_key && devices.some((d) => d.id.startsWith(k.match_key));
  const vendors = useMemo(() => [...new Set(db.map((d) => d.vendor))], [db]);
  const rows = useMemo(() => {
    const q = query.trim().toLowerCase();
    return db
      .filter((d) => vendor === "all" || d.vendor === vendor)
      .filter((d) => (support === "all" ? true : support === "connected" ? isConnected(d) : d.support === support))
      .filter((d) => !q || `${d.vendor} ${d.name} ${d.ids}`.toLowerCase().includes(q))
      .sort((a, b) => Number(isConnected(b)) - Number(isConnected(a)));
  }, [db, query, support, vendor, devices]);

  const chip = (id: typeof support, label: string) => (
    <button
      key={id}
      type="button"
      onClick={() => setSupport(id)}
      className={`rounded-full border px-2.5 py-1 text-[11px] transition ${support === id ? "border-accent bg-accent/15 text-text" : "border-line text-muted hover:text-text"}`}
    >
      {label}
    </button>
  );

  return (
    <div className="mx-auto flex max-w-6xl flex-col gap-4">
      <div>
        <h2 className="text-lg font-semibold">{t("nav.database")}</h2>
        <p className="text-xs text-muted">{t("db.subtitle", { n: db.length })}</p>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <label className="flex min-w-60 flex-1 items-center gap-2 rounded-lg border border-line bg-surface px-3 py-2">
          <Search size={14} className="text-muted" />
          <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t("db.search")} className="w-full bg-transparent text-sm outline-none" />
        </label>
        <select value={vendor} onChange={(e) => setVendor(e.target.value)} className="rounded-lg border border-line bg-surface px-3 py-2 text-sm" aria-label={t("db.vendor")}>
          <option value="all">{t("db.vendor")}: {t("db.all")}</option>
          {vendors.map((v) => (
            <option key={v} value={v}>
              {v}
            </option>
          ))}
        </select>
      </div>
      <div className="flex flex-wrap gap-1.5">
        {chip("all", t("db.all"))}
        {chip("connected", t("db.connected"))}
        {SUPPORTS.map((s) => chip(s, t(`db.support.${s}` as Key)))}
      </div>

      <div className="overflow-hidden rounded-2xl border border-line bg-surface">
        <table className="w-full text-start text-[13px]">
          <thead className="border-b border-line text-[11px] text-muted uppercase">
            <tr>
              <th className="px-4 py-2.5 font-semibold">{t("db.type")}</th>
              <th className="px-2 py-2.5 font-semibold">{t("db.vendor")}</th>
              <th className="px-2 py-2.5 font-semibold">{t("prof.name")}</th>
              <th className="hidden px-2 py-2.5 font-semibold md:table-cell">{t("db.ids")}</th>
              <th className="px-4 py-2.5 font-semibold" />
            </tr>
          </thead>
          <tbody>
            {rows.map((d, i) => {
              const connected = isConnected(d);
              return (
                <tr key={`${d.vendor}-${d.name}-${d.ids}-${i}`} className={`border-b border-line/60 last:border-0 ${connected ? "bg-accent/10" : ""}`}>
                  <td className="px-4 py-2 text-muted">
                    <DeviceIcon type={d.kind} size={16} />
                  </td>
                  <td className="px-2 py-2 text-muted">{d.vendor}</td>
                  <td className="px-2 py-2">
                    <span className="flex items-center gap-1.5 font-medium">
                      {d.name}
                      {connected && <CheckCircle2 size={14} className="text-accent" aria-label={t("db.connected")} />}
                    </span>
                    {d.note && <span className="block text-[11px] text-muted">{note(d.note)}</span>}
                  </td>
                  <td className="hidden px-2 py-2 font-mono text-[11px] text-muted md:table-cell">{d.ids}</td>
                  <td className="px-4 py-2 text-end">
                    <span className={`inline-block rounded-full border px-2 py-0.5 text-[10px] font-semibold whitespace-nowrap ${SUPPORT_TONE[d.support]}`}>
                      {t(`db.support.${d.support}` as Key)}
                    </span>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
        {rows.length === 0 && <p className="p-6 text-center text-sm text-muted">{t("db.noResults")}</p>}
      </div>
      <p className="text-[11px] leading-relaxed text-muted">{t("db.legend")}</p>
    </div>
  );
}
