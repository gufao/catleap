import { useState, useEffect } from "react";
import { scanSteam, checkWineStatus } from "../lib/tauri";
import type { WineStatus } from "../types";

interface FirstRunProps {
  onComplete: () => void;
}

const VARIANT_LABELS: Record<string, string> = {
  gptk: "Apple Game Porting Toolkit",
  "wine-crossover": "Wine CrossOver",
  crossover: "CrossOver",
  "homebrew-wine": "Wine (Homebrew)",
  wine: "Wine",
};

export function FirstRun({ onComplete }: FirstRunProps) {
  const [wineStatus, setWineStatus] = useState<WineStatus | null>(null);
  const [checking, setChecking] = useState(true);
  const [scanning, setScanning] = useState(false);
  const [done, setDone] = useState(false);
  const [gameCount, setGameCount] = useState(0);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    checkWineStatus()
      .then(setWineStatus)
      .catch(() =>
        setWineStatus({
          installed: false,
          variant: "none",
          path: "",
          homebrew_available: false,
        })
      )
      .finally(() => setChecking(false));
  }, []);

  async function handleRecheck() {
    setChecking(true);
    setError(null);
    try {
      const status = await checkWineStatus();
      setWineStatus(status);
    } catch {
      setError("Failed to check Wine status");
    } finally {
      setChecking(false);
    }
  }

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

  if (checking) {
    return (
      <div className="flex h-screen w-screen items-center justify-center bg-gray-50">
        <div className="flex flex-col items-center text-center">
          <span className="text-5xl mb-4 select-none">🐱</span>
          <p className="text-sm text-gray-400">Checking system...</p>
        </div>
      </div>
    );
  }

  const wineInstalled = wineStatus?.installed ?? false;
  const variantLabel =
    VARIANT_LABELS[wineStatus?.variant ?? ""] ?? wineStatus?.variant ?? "";

  return (
    <div className="flex h-screen w-screen items-center justify-center bg-gray-50">
      <div className="flex flex-col items-center text-center max-w-md px-6">
        <span className="text-6xl mb-6 select-none" role="img" aria-label="cat">
          🐱
        </span>

        <h1 className="text-3xl font-bold text-gray-900 mb-2">
          Welcome to Catleap
        </h1>
        <p className="text-base text-gray-500 mb-8">
          Play your Windows games on Mac.
        </p>

        {error && (
          <div className="rounded-lg bg-red-50 border border-red-100 px-4 py-3 text-sm text-red-700 mb-6 w-full text-left">
            {error}
          </div>
        )}

        {!wineInstalled ? (
          /* --- Wine/GPTK not found --- */
          <div className="w-full space-y-4">
            <div className="rounded-xl bg-amber-50 border border-amber-100 p-4 text-left">
              <p className="text-sm font-semibold text-amber-800 mb-2">
                Wine/GPTK not found
              </p>
              <p className="text-sm text-amber-700 mb-3">
                Catleap needs Apple's Game Porting Toolkit to run Windows games.
                Install it via Homebrew:
              </p>
              <div className="bg-gray-900 rounded-lg p-3 font-mono text-xs text-green-400 select-all">
                {wineStatus?.homebrew_available ? (
                  <>brew install --no-quarantine gcenx/wine/game-porting-toolkit</>
                ) : (
                  <>
                    <span className="text-gray-500"># Install Homebrew first:</span>
                    <br />
                    /bin/bash -c "$(curl -fsSL
                    https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
                    <br />
                    <br />
                    <span className="text-gray-500"># Then install GPTK:</span>
                    <br />
                    brew install --no-quarantine gcenx/wine/game-porting-toolkit
                  </>
                )}
              </div>
              <p className="text-xs text-amber-600 mt-3">
                This may take a while. After installing, click the button below.
              </p>
            </div>

            <button
              onClick={handleRecheck}
              disabled={checking}
              className="w-full px-5 py-3 rounded-xl bg-gray-900 text-white font-semibold text-sm hover:bg-gray-700 disabled:opacity-50 transition-colors"
            >
              {checking ? "Checking..." : "I've installed it — check again"}
            </button>

            <button
              onClick={onComplete}
              className="w-full px-5 py-3 rounded-xl bg-white border border-gray-200 text-gray-500 font-medium text-sm hover:bg-gray-50 transition-colors"
            >
              Skip for now
            </button>
          </div>
        ) : !done ? (
          /* --- Wine found, ready to scan --- */
          <div className="w-full space-y-4">
            <div className="rounded-xl bg-green-50 border border-green-100 p-3 text-left">
              <p className="text-sm text-green-800">
                <span className="font-semibold">Ready!</span> Using{" "}
                <span className="font-mono text-xs">{variantLabel}</span>
              </p>
            </div>

            <button
              onClick={handleScan}
              disabled={scanning}
              className="w-full px-5 py-3 rounded-xl bg-gray-900 text-white font-semibold text-sm hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              {scanning ? (
                <span className="flex items-center justify-center gap-2">
                  <span className="w-4 h-4 border-2 border-white/40 border-t-white rounded-full animate-spin" />
                  Scanning...
                </span>
              ) : (
                "Scan for Games"
              )}
            </button>

            <p className="text-xs text-gray-400">
              You can also add games manually later.
            </p>
          </div>
        ) : (
          /* --- Scan complete --- */
          <div className="w-full space-y-4">
            <p className="text-sm text-gray-600">
              Found{" "}
              <span className="font-semibold text-gray-900">{gameCount}</span>{" "}
              {gameCount === 1 ? "game" : "games"} in your Steam library.
            </p>

            <button
              onClick={onComplete}
              className="w-full px-5 py-3 rounded-xl bg-gray-900 text-white font-semibold text-sm hover:bg-gray-700 transition-colors"
            >
              Go to Library
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
