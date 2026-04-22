import { useEffect, useState, useMemo } from "react";
import { scanSteam, listGames } from "../lib/tauri";
import type { Game, GameStatus, GameSource } from "../types";

interface UseGamesOptions {
  statusFilter?: GameStatus | "all";
  sourceFilter?: GameSource | "all";
  searchQuery?: string;
}

interface UseGamesResult {
  games: Game[];
  loading: boolean;
  error: string | null;
  setGames: React.Dispatch<React.SetStateAction<Game[]>>;
}

export function useGames({
  statusFilter = "all",
  sourceFilter = "all",
  searchQuery = "",
}: UseGamesOptions = {}): UseGamesResult {
  const [allGames, setAllGames] = useState<Game[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      setLoading(true);
      setError(null);
      try {
        const games = await scanSteam();
        if (!cancelled) setAllGames(games);
      } catch (scanErr) {
        // Graceful fallback to listGames if scan fails
        console.warn("Steam scan failed, falling back to listGames:", scanErr);
        try {
          const games = await listGames();
          if (!cancelled) setAllGames(games);
        } catch (listErr) {
          if (!cancelled) {
            setError(String(listErr));
          }
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    load();
    return () => {
      cancelled = true;
    };
  }, []);

  const games = useMemo(() => {
    let filtered = allGames;

    if (statusFilter !== "all") {
      filtered = filtered.filter((g) => g.status === statusFilter);
    }

    if (sourceFilter !== "all") {
      filtered = filtered.filter((g) => g.source === sourceFilter);
    }

    if (searchQuery.trim()) {
      const q = searchQuery.trim().toLowerCase();
      filtered = filtered.filter((g) => g.name.toLowerCase().includes(q));
    }

    return filtered;
  }, [allGames, statusFilter, sourceFilter, searchQuery]);

  return { games, loading, error, setGames: setAllGames };
}
