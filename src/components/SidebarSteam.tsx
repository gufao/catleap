import { useSteamRuntime } from "../hooks/useSteamRuntime";

export function SidebarSteam() {
  const { state, startInstall, cancelInstall, open, stop } = useSteamRuntime();

  if (state.kind === "loading") {
    return null;
  }

  if (state.kind === "not_installed") {
    return (
      <div className="px-1 mb-4">
        <button
          onClick={startInstall}
          className="w-full px-3 py-2 rounded-md bg-gray-900 text-white text-sm font-semibold hover:bg-gray-700 transition-colors"
        >
          Install Steam
        </button>
        <p className="text-xs text-gray-400 mt-1.5 px-1">
          Run Windows-only Steam games on your Mac.
        </p>
      </div>
    );
  }

  if (state.kind === "installing") {
    const label =
      state.phase.kind === "initializing_prefix" ? "Setting up Wine..." :
      state.phase.kind === "installing_mono" ? "Installing Mono..." :
      state.phase.kind === "installing_gecko" ? "Installing Gecko..." :
      state.phase.kind === "configuring_prefix" ? "Configuring..." :
      state.phase.kind === "downloading_installer" ?
        `Downloading Steam... ${state.phase.bytes_total > 0
          ? Math.round((state.phase.bytes_done / state.phase.bytes_total) * 100)
          : 0}%` :
      state.phase.kind === "launching_installer" ? "Running Steam installer..." :
      state.phase.kind === "done" ? "Done." :
      `Failed: ${state.phase.error}`;

    return (
      <div className="px-1 mb-4">
        <div className="px-3 py-2 rounded-md bg-gray-100 flex items-center gap-2">
          <span className="w-3 h-3 border-2 border-gray-300 border-t-gray-700 rounded-full animate-spin" />
          <span className="text-sm text-gray-700 truncate">{label}</span>
        </div>
        <button
          onClick={cancelInstall}
          className="w-full mt-1 px-3 py-1 text-xs text-gray-500 hover:text-gray-800"
        >
          Cancel
        </button>
      </div>
    );
  }

  // installed
  return (
    <div className="px-1 mb-4">
      {state.running ? (
        <div className="flex items-center gap-2">
          <span className="flex-1 px-3 py-2 rounded-md bg-green-50 border border-green-200 text-sm text-green-800">
            Steam running
          </span>
          <button
            onClick={stop}
            className="px-2 py-2 text-xs text-gray-500 hover:text-red-600"
            title="Stop Steam"
          >
            ⏹
          </button>
        </div>
      ) : (
        <button
          onClick={open}
          className="w-full px-3 py-2 rounded-md bg-gray-900 text-white text-sm font-semibold hover:bg-gray-700 transition-colors"
        >
          Open Steam
        </button>
      )}
    </div>
  );
}
