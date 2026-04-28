import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  cancelSteamInstall,
  getSettings,
  launchSteamRuntime,
  startSteamInstall,
  stopSteamRuntime,
} from "../lib/tauri";
import { useTauriEvent } from "./useTauriEvent";
import type { Settings, SteamInstallPhase } from "../types";

export type SteamUiState =
  | { kind: "loading" }
  | { kind: "not_installed" }
  | { kind: "installing"; phase: SteamInstallPhase }
  | { kind: "installed"; running: boolean };

const STEAM_RUNTIME_ID = "_steam_runtime";

export function useSteamRuntime() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [phase, setPhase] = useState<SteamInstallPhase | null>(null);
  const [installing, setInstalling] = useState(false);
  const [running, setRunning] = useState(false);

  useEffect(() => {
    getSettings().then(setSettings).catch(() => {});
  }, []);

  // Subscribe to install progress while installing
  useTauriEvent<SteamInstallPhase>(
    "steam-install-progress",
    (p) => {
      setPhase(p);
      if (p.kind === "done") {
        setInstalling(false);
        getSettings().then(setSettings).catch(() => {});
      }
      if (p.kind === "failed") {
        setInstalling(false);
      }
    },
    installing
  );

  // Poll running status while we think Steam might be running
  useEffect(() => {
    if (!settings?.steam_runtime_installed) return;
    let stopped = false;
    const tick = async () => {
      try {
        const ids = (await invoke<string[]>("get_running_games")) ?? [];
        if (!stopped) setRunning(ids.includes(STEAM_RUNTIME_ID));
      } catch {}
    };
    tick();
    const id = setInterval(tick, 3000);
    return () => { stopped = true; clearInterval(id); };
  }, [settings?.steam_runtime_installed]);

  const startInstall = async () => {
    setInstalling(true);
    setPhase({ kind: "initializing_prefix" });
    try {
      await startSteamInstall();
    } catch (e) {
      setInstalling(false);
      setPhase({ kind: "failed", error: String(e) });
    }
  };
  const cancelInstall = () => cancelSteamInstall().catch(() => {});
  const open = () => launchSteamRuntime().catch((e) => alert(`Failed to open Steam: ${e}`));
  const stop = () => stopSteamRuntime().catch(() => {});

  let state: SteamUiState;
  if (!settings) state = { kind: "loading" };
  else if (installing && phase) state = { kind: "installing", phase };
  else if (settings.steam_runtime_installed) state = { kind: "installed", running };
  else state = { kind: "not_installed" };

  return { state, startInstall, cancelInstall, open, stop };
}
