import { useState, useCallback, useEffect, useRef } from "react";
import { playGame, stopGame } from "../lib/tauri";
import { invoke } from "@tauri-apps/api/core";

export function useLauncher(onStatusChange: () => void) {
  const [launching, setLaunching] = useState<string | null>(null);
  const pollRef = useRef<ReturnType<typeof setInterval> | undefined>(undefined);

  const play = useCallback(async (gameId: string) => {
    try {
      setLaunching(gameId);
      await playGame(gameId);
      onStatusChange();
    } catch (e) {
      console.error("Failed to launch game:", e);
      alert(`Failed to launch: ${e}`);
    } finally {
      setLaunching(null);
    }
  }, [onStatusChange]);

  const stop = useCallback(async (gameId: string) => {
    try {
      await stopGame(gameId);
      onStatusChange();
    } catch (e) {
      console.error("Failed to stop game:", e);
    }
  }, [onStatusChange]);

  // Poll running status every 3s. Only trigger a parent refresh when the
  // set of running games actually changes — otherwise the parent's
  // refreshKey-based remount causes a visible flicker every poll.
  const lastRunningRef = useRef<string>("");
  useEffect(() => {
    pollRef.current = setInterval(async () => {
      try {
        const ids = await invoke<string[]>("get_running_games");
        const sig = [...ids].sort().join(",");
        if (sig !== lastRunningRef.current) {
          lastRunningRef.current = sig;
          onStatusChange();
        }
      } catch {}
    }, 3000);
    return () => { if (pollRef.current) clearInterval(pollRef.current); };
  }, [onStatusChange]);

  return { play, stop, launching };
}
