import { invoke } from "@tauri-apps/api/core";
import type { Game, Settings } from "../types";

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
