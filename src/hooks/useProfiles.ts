import { useCallback, useEffect, useState } from "react";
import { getBackend } from "../lib/backend";
import type { Profile, ProfilesView } from "../lib/types";

const EMPTY: ProfilesView = { profiles: [], default_profile: null, auto_switch: true, active: { id: null, game: null } };

export function useProfiles() {
  const [view, setView] = useState<ProfilesView>(EMPTY);

  const reload = useCallback(async () => setView(await (await getBackend()).getProfiles()), []);

  useEffect(() => {
    let off: (() => void) | undefined;
    reload();
    getBackend().then(async (b) => {
      off = await b.onProfile((active) => setView((v) => ({ ...v, active })));
    });
    return () => off?.();
  }, [reload]);

  const run = useCallback(
    async (f: (b: Awaited<ReturnType<typeof getBackend>>) => Promise<unknown>) => {
      await f(await getBackend());
      await reload();
    },
    [reload],
  );

  return {
    view,
    save: (p: Profile) => run((b) => b.saveProfile(p)),
    remove: (id: string) => run((b) => b.deleteProfile(id)),
    apply: (id: string) => run((b) => b.applyProfile(id)),
    setDefault: (id: string | null) => run((b) => b.setDefaultProfile(id)),
    setAutoSwitch: (on: boolean) => run((b) => b.setAutoSwitch(on)),
  };
}

export function newProfileId(): string {
  return `p-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 6)}`;
}
