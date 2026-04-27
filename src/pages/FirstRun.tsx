import { useEffect, useState } from "react";
import {
  cancelWineInstall,
  getSettings,
  scanSteam,
  skipGptk,
  startGptkWatch,
  startWineInstall,
  stopGptkWatch,
} from "../lib/tauri";
import { useTauriEvent } from "../hooks/useTauriEvent";
import type {
  GptkImportPhase,
  Settings,
  WineInstallPhase,
} from "../types";

interface FirstRunProps {
  onComplete: () => void;
}

type Step = "welcome" | "wine" | "gptk" | "scan" | "done";

export function FirstRun({ onComplete }: FirstRunProps) {
  const [step, setStep] = useState<Step>("welcome");
  const [settings, setSettings] = useState<Settings | null>(null);
  const [winePhase, setWinePhase] = useState<WineInstallPhase | null>(null);
  const [gptkPhase, setGptkPhase] = useState<GptkImportPhase | null>(null);
  const [foundVolume, setFoundVolume] = useState<string | null>(null);
  const [scanResult, setScanResult] = useState<number | null>(null);
  const [scanning, setScanning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Resume mid-onboarding based on persisted settings.
  useEffect(() => {
    getSettings().then((s) => {
      setSettings(s);
      if (s.wine_version && (s.gptk_version || s.gptk_skipped)) {
        setStep("scan");
      } else if (s.wine_version) {
        setStep("gptk");
      } else {
        setStep("welcome");
      }
    });
  }, []);

  useTauriEvent<WineInstallPhase>(
    "wine-install-progress",
    (p) => {
      setWinePhase(p);
      if (p.kind === "done") setStep("gptk");
      if (p.kind === "failed") setError(p.error);
    },
    step === "wine"
  );

  useTauriEvent<GptkImportPhase>(
    "gptk-import-progress",
    (p) => {
      setGptkPhase(p);
      if (p.kind === "found") setFoundVolume(p.version);
      if (p.kind === "done") {
        setStep("scan");
        setFoundVolume(null);
      }
      if (p.kind === "failed") setError(p.error);
    },
    step === "gptk"
  );

  function startWine() {
    setError(null);
    setWinePhase({ kind: "checking_space" });
    startWineInstall().catch((e) => setError(String(e)));
  }

  function startGptk() {
    setError(null);
    setGptkPhase({ kind: "waiting" });
    startGptkWatch().catch((e) => setError(String(e)));
  }

  async function handleSkipGptk() {
    setError(null);
    try {
      await stopGptkWatch();
      await skipGptk();
      setStep("scan");
    } catch (e) {
      setError(String(e));
    }
  }

  async function runScan() {
    setScanning(true);
    setError(null);
    try {
      const games = await scanSteam();
      setScanResult(games.length);
      setStep("done");
    } catch (e) {
      setError(String(e));
    } finally {
      setScanning(false);
    }
  }

  if (!settings) {
    return (
      <Centered>
        <p className="text-sm text-gray-400">Loading...</p>
      </Centered>
    );
  }

  return (
    <Centered>
      <span className="text-6xl mb-6 select-none" role="img" aria-label="cat">🐱</span>

      {error && (
        <div className="rounded-lg bg-red-50 border border-red-100 px-4 py-3 text-sm text-red-700 mb-6 w-full text-left">
          {error}
        </div>
      )}

      {step === "welcome" && (
        <Welcome onContinue={() => setStep("wine")} />
      )}

      {step === "wine" && (
        <WineStep
          phase={winePhase}
          onStart={startWine}
          onCancel={() => cancelWineInstall().catch(() => {})}
          onRetry={startWine}
        />
      )}

      {step === "gptk" && (
        <GptkStep
          phase={gptkPhase}
          foundVolume={foundVolume}
          onStart={startGptk}
          onSkip={handleSkipGptk}
        />
      )}

      {step === "scan" && (
        <ScanStep onScan={runScan} scanning={scanning} />
      )}

      {step === "done" && (
        <DoneStep count={scanResult ?? 0} onComplete={onComplete} />
      )}
    </Centered>
  );
}

function Centered({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex h-screen w-screen items-center justify-center bg-gray-50">
      <div className="flex flex-col items-center text-center max-w-md px-6">
        {children}
      </div>
    </div>
  );
}

function Welcome({ onContinue }: { onContinue: () => void }) {
  return (
    <>
      <h1 className="text-3xl font-bold text-gray-900 mb-2">Welcome to Catleap</h1>
      <p className="text-base text-gray-500 mb-8">
        Play Windows games on Mac. We'll set up Wine and Apple's GPTK in two short steps.
      </p>
      <button
        onClick={onContinue}
        className="w-full px-5 py-3 rounded-xl bg-gray-900 text-white font-semibold text-sm hover:bg-gray-700 transition-colors"
      >
        Continue
      </button>
    </>
  );
}

function WineStep({
  phase,
  onStart,
  onCancel,
  onRetry,
}: {
  phase: WineInstallPhase | null;
  onStart: () => void;
  onCancel: () => void;
  onRetry: () => void;
}) {
  if (!phase) {
    return (
      <>
        <h2 className="text-2xl font-bold text-gray-900 mb-2">Download Wine</h2>
        <p className="text-base text-gray-500 mb-6">
          Catleap needs to download a custom Wine build (~150 MB) compiled from Apple's GPTK sources.
          One-time download.
        </p>
        <button
          onClick={onStart}
          className="w-full px-5 py-3 rounded-xl bg-gray-900 text-white font-semibold text-sm hover:bg-gray-700 transition-colors"
        >
          Download
        </button>
      </>
    );
  }

  const label =
    phase.kind === "checking_space" ? "Checking disk space..." :
    phase.kind === "downloading" ? `Downloading... ${phase.bytes_total > 0 ? Math.round((phase.bytes_done / phase.bytes_total) * 100) : 0}%` :
    phase.kind === "verifying" ? "Verifying..." :
    phase.kind === "extracting" ? "Extracting..." :
    phase.kind === "codesigning" ? "Signing binaries..." :
    phase.kind === "done" ? "Done." :
    `Failed: ${phase.error}`;

  const percent =
    phase.kind === "downloading" && phase.bytes_total > 0
      ? Math.round((phase.bytes_done / phase.bytes_total) * 100)
      : phase.kind === "verifying" || phase.kind === "extracting" || phase.kind === "codesigning" || phase.kind === "done"
      ? 100
      : 0;

  return (
    <>
      <h2 className="text-2xl font-bold text-gray-900 mb-4">Installing Wine</h2>
      <div className="w-full h-2 rounded-full bg-gray-200 overflow-hidden mb-3">
        <div
          className="h-full bg-gray-900 transition-all"
          style={{ width: `${percent}%` }}
        />
      </div>
      <p className="text-sm text-gray-600 mb-6">{label}</p>

      {phase.kind === "failed" ? (
        <button
          onClick={onRetry}
          className="w-full px-5 py-3 rounded-xl bg-gray-900 text-white font-semibold text-sm hover:bg-gray-700"
        >
          Retry
        </button>
      ) : (
        <button
          onClick={onCancel}
          className="w-full px-5 py-3 rounded-xl bg-white border border-gray-200 text-gray-500 font-medium text-sm hover:bg-gray-50"
        >
          Cancel
        </button>
      )}
    </>
  );
}

function GptkStep({
  phase,
  foundVolume,
  onStart,
  onSkip,
}: {
  phase: GptkImportPhase | null;
  foundVolume: string | null;
  onStart: () => void;
  onSkip: () => void;
}) {
  if (!phase) {
    return (
      <>
        <h2 className="text-2xl font-bold text-gray-900 mb-2">Apple GPTK Libraries</h2>
        <p className="text-base text-gray-500 mb-6">
          Download the Game Porting Toolkit DMG from Apple (free Apple ID required), then mount it.
          Catleap will detect it automatically.
        </p>
        <a
          href="https://developer.apple.com/games/game-porting-toolkit/"
          target="_blank"
          rel="noreferrer"
          className="w-full block text-center px-5 py-3 rounded-xl bg-white border border-gray-200 text-gray-900 font-medium text-sm hover:bg-gray-50 mb-3"
        >
          Open Apple Developer page
        </a>
        <button
          onClick={onStart}
          className="w-full px-5 py-3 rounded-xl bg-gray-900 text-white font-semibold text-sm hover:bg-gray-700 mb-3"
        >
          Start watching for DMG
        </button>
        <button
          onClick={onSkip}
          className="w-full px-5 py-3 rounded-xl bg-transparent text-gray-500 font-medium text-sm hover:bg-gray-100"
        >
          Skip — performance will be limited
        </button>
      </>
    );
  }

  const label =
    phase.kind === "waiting" ? "Waiting for GPTK DMG..." :
    phase.kind === "found" ? `Found GPTK ${phase.version}` :
    phase.kind === "copying" ? "Copying libraries..." :
    phase.kind === "done" ? `GPTK ${phase.version} installed.` :
    `Failed: ${phase.error}`;

  return (
    <>
      <h2 className="text-2xl font-bold text-gray-900 mb-4">Importing GPTK</h2>
      <p className="text-sm text-gray-600 mb-6">{label}</p>
      {foundVolume && phase.kind !== "done" && phase.kind !== "failed" ? null : null}
      <button
        onClick={onSkip}
        className="w-full px-5 py-3 rounded-xl bg-transparent text-gray-500 font-medium text-sm hover:bg-gray-100"
      >
        Skip
      </button>
    </>
  );
}

function ScanStep({ onScan, scanning }: { onScan: () => void; scanning: boolean }) {
  return (
    <>
      <h2 className="text-2xl font-bold text-gray-900 mb-2">Scan your Steam library</h2>
      <p className="text-base text-gray-500 mb-6">
        Catleap will look for installed Steam games. You can also add games manually later.
      </p>
      <button
        onClick={onScan}
        disabled={scanning}
        className="w-full px-5 py-3 rounded-xl bg-gray-900 text-white font-semibold text-sm hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed"
      >
        {scanning ? "Scanning..." : "Scan for Games"}
      </button>
    </>
  );
}

function DoneStep({ count, onComplete }: { count: number; onComplete: () => void }) {
  return (
    <>
      <h2 className="text-2xl font-bold text-gray-900 mb-2">All set</h2>
      <p className="text-base text-gray-500 mb-6">
        Found <span className="font-semibold text-gray-900">{count}</span> {count === 1 ? "game" : "games"}.
      </p>
      <button
        onClick={onComplete}
        className="w-full px-5 py-3 rounded-xl bg-gray-900 text-white font-semibold text-sm hover:bg-gray-700"
      >
        Go to Library
      </button>
    </>
  );
}
