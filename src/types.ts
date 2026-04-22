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
}
