import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";
import { api, isTauri } from "../providers/api";
import type { AppError, Settings } from "../types/translation";

export function useSettings() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [error, setError] = useState<AppError | null>(null);

  useEffect(() => {
    if (!isTauri() && !import.meta.env.DEV) return;
    api.getSettings().then(setSettings).catch(setError);
    if (!isTauri()) return;
    const un = listen<Settings>("settings:changed", (e) => setSettings(e.payload));
    return () => {
      un.then((f) => f());
    };
  }, []);

  const update = useCallback(
    async (patch: Partial<Settings>) => {
      if (!settings) return;
      try {
        const next = await api.updateSettings({ ...settings, ...patch });
        setSettings(next);
        setError(null);
        return next;
      } catch (e) {
        setError(e as AppError);
        throw e;
      }
    },
    [settings],
  );

  return { settings, update, error, setError };
}
