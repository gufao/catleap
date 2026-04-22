import { useState } from "react";
import { Sidebar } from "./components/Sidebar";
import type { LibraryFilter, SourceFilter } from "./components/Sidebar";

type Page = "library" | "settings";

function App() {
  const [page, setPage] = useState<Page>("library");
  const [libraryFilter, setLibraryFilter] = useState<LibraryFilter>("all");
  const [sourceFilter, setSourceFilter] = useState<SourceFilter>("all");

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
          <div className="p-8">
            <p className="text-gray-400 text-sm">
              Library — filter: {libraryFilter} / source: {sourceFilter}
            </p>
          </div>
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
