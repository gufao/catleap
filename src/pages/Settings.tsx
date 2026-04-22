import { useState, useEffect } from "react";
import { useSettings } from "../hooks/useSettings";

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
