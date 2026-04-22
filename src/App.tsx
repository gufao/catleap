import { useState, useCallback } from "react";
import { Sidebar } from "./components/Sidebar";
import type { LibraryFilter, SourceFilter } from "./components/Sidebar";
import { Library } from "./pages/Library";
import { useLauncher } from "./hooks/useLauncher";

type Page = "library" | "settings";

function App() {
  const [page, setPage] = useState<Page>("library");
  const [libraryFilter, setLibraryFilter] = useState<LibraryFilter>("all");
  const [sourceFilter, setSourceFilter] = useState<SourceFilter>("all");
  const [refreshKey, setRefreshKey] = useState(0);

  const handleStatusChange = useCallback(() => {
    setRefreshKey((k) => k + 1);
  }, []);

  const { play, stop } = useLauncher(handleStatusChange);

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
          />
        )}
        {page === "settings" && (
          <div className="p-8">
            <h1 className="text-2xl font-semibold text-gray-900 mb-1">Settings</h1>
            <p className="text-gray-400 text-sm">Settings page coming soon.</p>
          </div>
        )}
      </main>
    </div>
  );
}

export default App;
