import { useState } from "react";
import { scanSteam } from "../lib/tauri";

interface FirstRunProps {
  onComplete: () => void;
}

export function FirstRun({ onComplete }: FirstRunProps) {
  const [scanning, setScanning] = useState(false);
  const [done, setDone] = useState(false);
  const [gameCount, setGameCount] = useState(0);
  const [error, setError] = useState<string | null>(null);

  async function handleScan() {
    setScanning(true);
    setError(null);
    try {
      const games = await scanSteam();
      setGameCount(games.length);
      setDone(true);
    } catch (e) {
      setError(String(e));
    } finally {
      setScanning(false);
    }
  }

  return (
    <div className="flex h-screen w-screen items-center justify-center bg-gray-50">
      <div className="flex flex-col items-center text-center max-w-sm px-6">
        <span className="text-6xl mb-6 select-none" role="img" aria-label="cat">
          🐱
        </span>

        <h1 className="text-3xl font-bold text-gray-900 mb-2">Welcome to Catleap</h1>
        <p className="text-base text-gray-500 mb-8">Play your Windows games on Mac.</p>

        {error && (
          <div className="rounded-lg bg-red-50 border border-red-100 px-4 py-3 text-sm text-red-700 mb-6 w-full text-left">
            {error}
          </div>
        )}

        {!done ? (
          <>
            <button
              onClick={handleScan}
              disabled={scanning}
              className="w-full px-5 py-3 rounded-xl bg-gray-900 text-white font-semibold text-sm hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors mb-4"
            >
              {scanning ? (
                <span className="flex items-center justify-center gap-2">
                  <span className="w-4 h-4 border-2 border-white/40 border-t-white rounded-full animate-spin" />
                  Scanning…
                </span>
              ) : (
                "Scan for Games"
              )}
            </button>
            <p className="text-xs text-gray-400">
              You can also add games manually later.
            </p>
          </>
        ) : (
          <>
            <p className="text-sm text-gray-600 mb-6">
              Found{" "}
              <span className="font-semibold text-gray-900">{gameCount}</span>{" "}
              {gameCount === 1 ? "game" : "games"} in your Steam library.
            </p>
            <button
              onClick={onComplete}
              className="w-full px-5 py-3 rounded-xl bg-gray-900 text-white font-semibold text-sm hover:bg-gray-700 transition-colors mb-4"
            >
              Go to Library
            </button>
            <p className="text-xs text-gray-400">
              You can also add games manually later.
            </p>
          </>
        )}
      </div>
    </div>
  );
}
