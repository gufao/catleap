export type GameStatus = "compatible" | "experimental" | "incompatible" | "unknown";
export type GameSource = "steam" | "manual";

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
}

export interface WineStatus {
  installed: boolean;
  variant: string;
  path: string;
  gptk_libs_installed: boolean;
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
