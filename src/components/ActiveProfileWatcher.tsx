import { useEffect, useRef } from "react";
import { useI18n } from "../i18n";
import type { ProfilesView } from "../lib/types";
import { useContextMenu } from "./ContextMenu";

/** Shows a short notice when a profile becomes active (manually or for a game). */
export function ActiveProfileWatcher({ profiles }: { profiles: ProfilesView }) {
  const { t } = useI18n();
  const ctx = useContextMenu();
  const last = useRef<string | null>(null);
  useEffect(() => {
    const { id, game } = profiles.active;
    const key = id ? `${id}|${game ?? ""}` : null;
    if (key && last.current !== null && key !== last.current) {
      const name = profiles.profiles.find((p) => p.id === id)?.name ?? "";
      ctx.notify(game ? `${name} · ${t("prof.activeGame", { game })}` : `${t("prof.applied")}: ${name}`);
    }
    last.current = key ?? "";
  }, [profiles, ctx, t]);
  return null;
}
