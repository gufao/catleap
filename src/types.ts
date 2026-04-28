export type GameStatus = "compatible" | "experimental" | "incompatible" | "unknown";
export type GameSource = "steam" | "steam_wine" | "manual";

export interface Game {
  id: string;
  name: string;
  source: GameSource;
  status: GameStatus;
  install_dir: string;
  executable: string | null;
  size_bytes: number | null;
  is_running: boolean;
  notes: string | null;
}

export interface Settings {
  steam_path: string;
  data_path: string;
  wine_version: string | null;
  gptk_version: string | null;
  gptk_skipped: boolean;
  steam_runtime_installed: boolean;
}

export interface WineStatus {
  installed: boolean;
  variant: string;
  path: string;
  gptk_libs_installed: boolean;
  installed_version: string | null;
  expected_version: string;
}

export type WineInstallPhase =
  | { kind: "checking_space" }
  | { kind: "downloading"; bytes_done: number; bytes_total: number }
  | { kind: "verifying" }
  | { kind: "extracting" }
  | { kind: "codesigning" }
  | { kind: "done" }
  | { kind: "failed"; error: string };

export type GptkImportPhase =
  | { kind: "waiting" }
  | { kind: "found"; version: string }
  | { kind: "copying" }
  | { kind: "done"; version: string }
  | { kind: "failed"; error: string };

export type SteamInstallPhase =
  | { kind: "initializing_prefix" }
  | { kind: "installing_mono" }
  | { kind: "installing_gecko" }
  | { kind: "configuring_prefix" }
  | { kind: "downloading_installer"; bytes_done: number; bytes_total: number }
  | { kind: "launching_installer" }
  | { kind: "done" }
  | { kind: "failed"; error: string };
