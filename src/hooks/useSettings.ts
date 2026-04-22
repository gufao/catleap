import { useEffect, useState, useCallback } from "react";
import { getSettings, updateSettings } from "../lib/tauri";
import type { Settings } from "../types";

interface UseSettingsResult {
  settings: Settings | null;
  loading: boolean;
  error: string | null;
  save: (settings: Settings) => Promise<void>;
}

export function useSettings(): UseSettingsResult {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      setLoading(true);
      setError(null);
      try {
        const s = await getSettings();
        if (!cancelled) setSettings(s);
      } catch (e) {
        if (!cancelled) setError(String(e));
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    load();
    return () => {
      cancelled = true;
    };
  }, []);

  const save = useCallback(async (newSettings: Settings) => {
    await updateSettings(newSettings);
    setSettings(newSettings);
  }, []);

  return { settings, loading, error, save };
}
