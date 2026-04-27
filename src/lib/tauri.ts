import { invoke } from "@tauri-apps/api/core";
import type { Game, Settings, WineStatus } from "../types";

export function listGames(): Promise<Game[]> {
  return invoke<Game[]>("list_games");
}

export function scanSteam(): Promise<Game[]> {
  return invoke<Game[]>("scan_steam");
}

export function addManualGame(name: string, executablePath: string): Promise<Game> {
  return invoke<Game>("add_manual_game", { name, executablePath });
}

export function removeGame(gameId: string): Promise<void> {
  return invoke<void>("remove_game", { gameId });
}

export function playGame(gameId: string): Promise<void> {
  return invoke<void>("play_game", { gameId });
}

export function stopGame(gameId: string): Promise<void> {
  return invoke<void>("stop_game", { gameId });
}

export function getSettings(): Promise<Settings> {
  return invoke<Settings>("get_settings");
}

export function updateSettings(settings: Settings): Promise<void> {
  return invoke<void>("update_settings", { settings });
}

export function readGameLog(gameId: string): Promise<string> {
  return invoke<string>("read_game_log", { gameId });
}

export function checkWineStatus(): Promise<WineStatus> {
  return invoke<WineStatus>("check_wine_status");
}

export function startWineInstall(): Promise<void> {
  return invoke<void>("start_wine_install");
}

/**
 * Sets the install cancellation flag. The download loop checks this flag
 * between chunks and aborts. Promise resolution only confirms the flag
 * was set — it does NOT mean the download has stopped. Listen for a
 * `wine-install-progress` `Failed { error: "cancelled" }` event to know
 * when the install has actually halted.
 */
export function cancelWineInstall(): Promise<void> {
  return invoke<void>("cancel_wine_install");
}

/**
 * Begin watching `/Volumes` for an Apple GPTK DMG. If a watch is already
 * in progress, this is a no-op and resolves successfully. Listen for
 * `gptk-import-progress` events to track the actual state.
 */
export function startGptkWatch(): Promise<void> {
  return invoke<void>("start_gptk_watch");
}

export function stopGptkWatch(): Promise<void> {
  return invoke<void>("stop_gptk_watch");
}

export function skipGptk(): Promise<void> {
  return invoke<void>("skip_gptk");
}

export function ejectGptkVolume(volumePath: string): Promise<void> {
  return invoke<void>("eject_gptk_volume", { volumePath });
}
