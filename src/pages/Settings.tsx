import { useState, useEffect } from "react";
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
import { useSettings } from "../hooks/useSettings";
import { getSettings, startGptkWatch, stopGptkWatch, resetSteamRuntime } from "../lib/tauri";
import { useTauriEvent } from "../hooks/useTauriEvent";
import { useSteamRuntime } from "../hooks/useSteamRuntime";
import type { GptkImportPhase, Settings } from "../types";

function GptkSection() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [phase, setPhase] = useState<GptkImportPhase | null>(null);
  const [importing, setImporting] = useState(false);

  useEffect(() => {
    getSettings().then(setSettings);
  }, []);

  useTauriEvent<GptkImportPhase>(
    "gptk-import-progress",
    (p) => {
      setPhase(p);
      if (p.kind === "done" || p.kind === "failed") {
        setImporting(false);
        getSettings().then(setSettings);
      }
    },
    importing
  );

  if (!settings) return null;

  const installed = !!settings.gptk_version;
  const skipped = settings.gptk_skipped;

  async function handleImport() {
    setImporting(true);
    setPhase({ kind: "waiting" });
    try {
      await startGptkWatch();
    } catch (e) {
      setImporting(false);
      setPhase({ kind: "failed", error: String(e) });
    }
  }

  async function handleCancel() {
    setImporting(false);
    await stopGptkWatch().catch(() => undefined);
    setPhase(null);
  }

  return (
    <section>
      <h2 className="text-xs font-semibold text-gray-400 uppercase tracking-wider mb-3">
        Game Porting Toolkit
      </h2>
      {installed && (
        <p className="text-sm text-gray-600 mb-2">
          Apple GPTK <span className="font-mono">{settings.gptk_version}</span> installed.
        </p>
      )}
      {!installed && skipped && !importing && (
        <div className="rounded-xl bg-amber-50 border border-amber-100 p-4 mb-3">
          <p className="text-sm text-amber-800 font-semibold mb-1">
            GPTK not installed — game performance is limited
          </p>
          <p className="text-sm text-amber-700">
            Mount Apple's GPTK DMG and click Import to enable D3DMetal.
          </p>
        </div>
      )}
      {importing && phase && (
        <p className="text-sm text-gray-600 mb-2">
          {phase.kind === "waiting" && "Waiting for GPTK DMG..."}
          {phase.kind === "found" && `Found GPTK ${phase.version}`}
          {phase.kind === "copying" && "Copying libraries..."}
          {phase.kind === "done" && `Imported GPTK ${phase.version}`}
          {phase.kind === "failed" && `Failed: ${phase.error}`}
        </p>
      )}
      <div className="flex gap-2">
        {!importing ? (
          <button
            onClick={handleImport}
            className="px-4 py-2 rounded-lg bg-gray-900 text-white text-sm font-semibold hover:bg-gray-700"
          >
            {installed ? "Re-import" : "Import GPTK"}
          </button>
        ) : (
          <button
            onClick={handleCancel}
            className="px-4 py-2 rounded-lg bg-white border border-gray-200 text-gray-600 text-sm hover:bg-gray-50"
          >
            Cancel
          </button>
        )}
      </div>
    </section>
  );
}

function SteamRuntimeSection() {
  const { state, startInstall } = useSteamRuntime();
  const [dataPath, setDataPath] = useState<string | null>(null);

  useEffect(() => {
    getSettings().then((s) => setDataPath(s.data_path));
  }, []);

  async function handleReset() {
    if (!window.confirm(
      "Delete the Steam runtime and all games installed inside it? This cannot be undone."
    )) return;
    try {
      await resetSteamRuntime();
      window.location.reload();
    } catch (e) {
      alert(`Failed to reset: ${e}`);
    }
  }

  async function openCatleapLogs() {
    if (!dataPath) return;
    await openPath(`${dataPath}/logs`).catch((e) => alert(`Failed: ${e}`));
  }

  async function openSteamLogs() {
    if (!dataPath) return;
    const steamLogs = `${dataPath}/prefixes/_steam_runtime/drive_c/Program Files (x86)/Steam/logs`;
    await openPath(steamLogs).catch((e) => alert(`Failed: ${e}`));
  }

  async function revealRuntimePrefix() {
    if (!dataPath) return;
    await revealItemInDir(`${dataPath}/prefixes/_steam_runtime`).catch((e) => alert(`Failed: ${e}`));
  }

  return (
    <section className="mt-8">
      <h2 className="text-lg font-semibold text-gray-900 mb-3">Steam Runtime</h2>
      {state.kind === "installed" && (
        <>
          <p className="text-sm text-gray-600 mb-3">
            Steam-Windows is installed. Use the Library to launch Windows games.
          </p>
          <div className="flex gap-2 flex-wrap mb-3">
            <button
              onClick={handleReset}
              className="px-4 py-2 rounded-lg bg-white border border-red-200 text-red-700 text-sm font-semibold hover:bg-red-50"
            >
              Reset Steam
            </button>
            <button
              onClick={startInstall}
              className="px-4 py-2 rounded-lg bg-white border border-gray-200 text-gray-700 text-sm font-semibold hover:bg-gray-50"
            >
              Reinstall
            </button>
          </div>
          <div className="border-t border-gray-100 pt-3">
            <p className="text-xs text-gray-400 uppercase tracking-wider mb-2">Debug</p>
            <div className="flex gap-2 flex-wrap">
              <button
                onClick={openCatleapLogs}
                className="px-3 py-1.5 rounded-md bg-white border border-gray-200 text-gray-600 text-xs hover:bg-gray-50"
              >
                Open Catleap logs
              </button>
              <button
                onClick={openSteamLogs}
                className="px-3 py-1.5 rounded-md bg-white border border-gray-200 text-gray-600 text-xs hover:bg-gray-50"
              >
                Open Steam logs
              </button>
              <button
                onClick={revealRuntimePrefix}
                className="px-3 py-1.5 rounded-md bg-white border border-gray-200 text-gray-600 text-xs hover:bg-gray-50"
              >
                Reveal prefix in Finder
              </button>
            </div>
          </div>
        </>
      )}
      {state.kind === "not_installed" && (
        <p className="text-sm text-gray-600">
          Steam runtime not installed. Use the sidebar to install it.
        </p>
      )}
      {state.kind === "installing" && (
        <p className="text-sm text-gray-600">
          Installation in progress... see the sidebar for status.
        </p>
      )}
    </section>
  );
}

interface SettingsPageProps {
  onBack: () => void;
}

export function SettingsPage({ onBack }: SettingsPageProps) {
  const { settings, loading, error, save } = useSettings();
  const [steamPath, setSteamPath] = useState("");
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    if (settings) {
      setSteamPath(settings.steam_path);
    }
  }, [settings]);

  async function handleSave() {
    if (!settings) return;
    setSaving(true);
    setSaveError(null);
    setSaved(false);
    try {
      await save({ ...settings, steam_path: steamPath });
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      setSaveError(String(e));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="p-8 max-w-2xl">
      <div className="flex items-center gap-3 mb-8">
        <button
          onClick={onBack}
          className="flex items-center gap-1.5 text-sm text-gray-500 hover:text-gray-800 transition-colors"
        >
          <svg
            className="w-4 h-4"
            fill="none"
            stroke="currentColor"
            strokeWidth={2}
            viewBox="0 0 24 24"
          >
            <path strokeLinecap="round" strokeLinejoin="round" d="M15.75 19.5 8.25 12l7.5-7.5" />
          </svg>
          Back to Library
        </button>
      </div>

      <h1 className="text-2xl font-semibold text-gray-900 mb-1">Settings</h1>
      <p className="text-sm text-gray-400 mb-8">Configure your Catleap installation.</p>

      {loading && (
        <div className="flex items-center gap-2 text-sm text-gray-400">
          <div className="w-4 h-4 border-2 border-gray-300 border-t-gray-600 rounded-full animate-spin" />
          Loading settings…
        </div>
      )}

      {error && !loading && (
        <div className="rounded-lg bg-red-50 border border-red-100 px-4 py-3 text-sm text-red-700 mb-6">
          Failed to load settings: {error}
        </div>
      )}

      {!loading && settings && (
        <div className="flex flex-col gap-8">
          {/* Steam */}
          <section>
            <h2 className="text-xs font-semibold text-gray-400 uppercase tracking-wider mb-3">
              Steam
            </h2>
            <div className="flex flex-col gap-1">
              <label className="text-sm font-medium text-gray-700" htmlFor="steam-path">
                Steam Library Path
              </label>
              <p className="text-xs text-gray-400 mb-1">
                Path to your Steam installation (typically ~/Library/Application Support/Steam).
              </p>
              <input
                id="steam-path"
                type="text"
                value={steamPath}
                onChange={(e) => setSteamPath(e.target.value)}
                className="w-full px-3 py-2 rounded-lg border border-gray-200 bg-white text-sm text-gray-900 focus:outline-none focus:ring-2 focus:ring-gray-900/10 focus:border-gray-400 transition"
                placeholder="/path/to/Steam"
              />
            </div>
          </section>

          {/* Data */}
          <section>
            <h2 className="text-xs font-semibold text-gray-400 uppercase tracking-wider mb-3">
              Data
            </h2>
            <div className="flex flex-col gap-1">
              <label className="text-sm font-medium text-gray-700">
                Data Directory
              </label>
              <p className="text-xs text-gray-400 mb-1">
                Where Catleap stores Wine prefixes, logs, and config. Read-only.
              </p>
              <input
                type="text"
                value={settings.data_path}
                readOnly
                className="w-full px-3 py-2 rounded-lg border border-gray-200 bg-gray-50 text-sm text-gray-500 cursor-not-allowed"
              />
            </div>
          </section>

          {/* GPTK */}
          <GptkSection />

          {/* Steam Runtime */}
          <SteamRuntimeSection />

          {/* Save */}
          <div className="flex items-center gap-3">
            <button
              onClick={handleSave}
              disabled={saving}
              className="px-4 py-2 rounded-lg bg-gray-900 text-white text-sm font-semibold hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              {saving ? "Saving…" : "Save Changes"}
            </button>
            {saved && (
              <span className="text-sm text-green-600 font-medium">Saved!</span>
            )}
            {saveError && (
              <span className="text-sm text-red-600">{saveError}</span>
            )}
          </div>

          {/* About */}
          <section>
            <h2 className="text-xs font-semibold text-gray-400 uppercase tracking-wider mb-3">
              About
            </h2>
            <div className="rounded-lg border border-gray-100 bg-white px-4 py-3 flex flex-col gap-1">
              <div className="flex items-center justify-between">
                <span className="text-sm text-gray-500">Version</span>
                <span className="text-sm font-medium text-gray-900">0.1.0</span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-sm text-gray-500">License</span>
                <span className="text-sm font-medium text-gray-900">MIT</span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-sm text-gray-500">Platform</span>
                <span className="text-sm font-medium text-gray-900">macOS</span>
              </div>
            </div>
          </section>
        </div>
      )}
    </div>
  );
}
