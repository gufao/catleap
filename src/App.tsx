import { useState, useCallback } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Sidebar } from "./components/Sidebar";
import type { LibraryFilter, SourceFilter } from "./components/Sidebar";
import { Library } from "./pages/Library";
import { SettingsPage } from "./pages/Settings";
import { FirstRun } from "./pages/FirstRun";
import { GameDetail } from "./components/GameDetail";
import { useLauncher } from "./hooks/useLauncher";
import { addManualGame } from "./lib/tauri";
import type { Game } from "./types";

const ONBOARDED_KEY = "catleap_onboarded";

type Page = "library" | "settings" | "detail";

function App() {
  const [firstRun, setFirstRun] = useState(() => {
    return localStorage.getItem(ONBOARDED_KEY) !== "true";
  });
  const [page, setPage] = useState<Page>("library");
  const [libraryFilter, setLibraryFilter] = useState<LibraryFilter>("all");
  const [sourceFilter, setSourceFilter] = useState<SourceFilter>("all");
  const [refreshKey, setRefreshKey] = useState(0);
  const [selectedGame, setSelectedGame] = useState<Game | null>(null);

  const handleStatusChange = useCallback(() => {
    setRefreshKey((k) => k + 1);
  }, []);

  const { play, stop } = useLauncher(handleStatusChange);

  const handleFirstRunComplete = useCallback(() => {
    localStorage.setItem(ONBOARDED_KEY, "true");
    setFirstRun(false);
    setPage("library");
  }, []);

  const handleAddGame = useCallback(async () => {
    try {
      const selected = await open({
        title: "Select Game Executable",
        filters: [{ name: "Executables", extensions: ["exe"] }],
        multiple: false,
        directory: false,
      });

      if (!selected || typeof selected !== "string") return;

      const parts = selected.split("/");
      const filename = parts[parts.length - 1];
      const defaultName = filename.replace(/\.exe$/i, "");

      const name = window.prompt("Game name:", defaultName);
      if (!name) return;

      await addManualGame(name, selected);
      setRefreshKey((k) => k + 1);
    } catch (e) {
      console.error("Failed to add game:", e);
      alert(`Failed to add game: ${e}`);
    }
  }, []);

  const handleSelectGame = useCallback((game: Game) => {
    setSelectedGame(game);
    setPage("detail");
  }, []);

  if (firstRun) {
    return <FirstRun onComplete={handleFirstRunComplete} />;
  }

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-white font-sans">
      <Sidebar
        libraryFilter={libraryFilter}
        sourceFilter={sourceFilter}
        onLibraryFilterChange={(f) => {
          setLibraryFilter(f);
          setPage("library");
        }}
        onSourceFilterChange={(f) => {
          setSourceFilter(f);
          setPage("library");
        }}
        onNavigateSettings={() => setPage("settings")}
      />

      <main className="flex-1 overflow-y-auto bg-gray-50">
        {page === "library" && (
          <Library
            key={refreshKey}
            libraryFilter={libraryFilter}
            sourceFilter={sourceFilter}
            onPlay={play}
            onStop={stop}
            onAddGame={handleAddGame}
            onSelectGame={handleSelectGame}
          />
        )}
        {page === "detail" && selectedGame && (
          <GameDetail
            game={selectedGame}
            onPlay={play}
            onStop={stop}
            onBack={() => setPage("library")}
          />
        )}
        {page === "settings" && (
          <SettingsPage onBack={() => setPage("library")} />
        )}
      </main>
    </div>
  );
}

export default App;
