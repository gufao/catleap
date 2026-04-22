import { useEffect, useState, useMemo, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
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

  const load = useCallback(async (cancelled: { value: boolean }) => {
    setLoading(true);
    setError(null);
    try {
      const games = await scanSteam();
      if (!cancelled.value) setAllGames(games);
    } catch (scanErr) {
      // Graceful fallback to listGames if scan fails
      console.warn("Steam scan failed, falling back to listGames:", scanErr);
      try {
        const games = await listGames();
        if (!cancelled.value) setAllGames(games);
      } catch (listErr) {
        if (!cancelled.value) {
          setError(String(listErr));
        }
      }
    } finally {
      if (!cancelled.value) setLoading(false);
    }
  }, []);

  useEffect(() => {
    const cancelled = { value: false };
    load(cancelled);
    return () => {
      cancelled.value = true;
    };
  }, [load]);

  // Listen for Steam library changes and refresh
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let active = true;

    listen<void>("steam-library-changed", () => {
      const cancelled = { value: false };
      load(cancelled);
    }).then((fn) => {
      if (active) {
        unlisten = fn;
      } else {
        fn(); // immediately unlisten if already unmounted
      }
    });

    return () => {
      active = false;
      if (unlisten) unlisten();
    };
  }, [load]);

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
