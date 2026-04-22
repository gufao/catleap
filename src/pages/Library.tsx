import { useState } from "react";
import { GameGrid } from "../components/GameGrid";
import { SearchBar } from "../components/SearchBar";
import { useGames } from "../hooks/useGames";
import type { LibraryFilter, SourceFilter } from "../components/Sidebar";
import type { GameStatus } from "../types";

interface LibraryProps {
  libraryFilter: LibraryFilter;
  sourceFilter: SourceFilter;
  onPlay: (gameId: string) => void;
  onStop: (gameId: string) => void;
}

function libraryFilterToStatus(f: LibraryFilter): GameStatus | "all" {
  if (f === "compatible") return "compatible";
  // "recent" would need play history — treat as "all" for now
  return "all";
}

export function Library({ libraryFilter, sourceFilter, onPlay, onStop }: LibraryProps) {
  const [searchQuery, setSearchQuery] = useState("");

  const { games, loading, error } = useGames({
    statusFilter: libraryFilterToStatus(libraryFilter),
    sourceFilter: sourceFilter === "all" ? "all" : sourceFilter,
    searchQuery,
  });

  function handleAddGame() {
    console.log("Add Game clicked — manual game addition coming soon");
  }

  return (
    <div className="p-8">
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-semibold text-gray-900 leading-none">Library</h1>
          {!loading && (
            <p className="text-sm text-gray-400 mt-1">
              {games.length} {games.length === 1 ? "game" : "games"}
            </p>
          )}
        </div>
        <SearchBar
          query={searchQuery}
          onQueryChange={setSearchQuery}
          onAddGame={handleAddGame}
        />
      </div>

      {loading && (
        <div className="flex items-center justify-center py-24">
          <div className="w-6 h-6 border-2 border-gray-300 border-t-gray-700 rounded-full animate-spin" />
        </div>
      )}

      {error && !loading && (
        <div className="rounded-lg bg-red-50 border border-red-100 px-4 py-3 text-sm text-red-700 mb-4">
          Failed to load games: {error}
        </div>
      )}

      {!loading && <GameGrid games={games} onPlay={onPlay} onStop={onStop} />}
    </div>
  );
}
