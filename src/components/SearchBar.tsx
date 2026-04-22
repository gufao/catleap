interface SearchBarProps {
  query: string;
  onQueryChange: (q: string) => void;
  onAddGame: () => void;
}

export function SearchBar({ query, onQueryChange, onAddGame }: SearchBarProps) {
  return (
    <div className="flex items-center gap-3">
      <div className="relative flex-1 max-w-xs">
        <svg
          className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-gray-400 pointer-events-none"
          fill="none"
          stroke="currentColor"
          strokeWidth={2}
          viewBox="0 0 24 24"
        >
          <circle cx="11" cy="11" r="8" />
          <path d="m21 21-4.35-4.35" />
        </svg>
        <input
          type="text"
          placeholder="Search games…"
          value={query}
          onChange={(e) => onQueryChange(e.currentTarget.value)}
          className="w-full pl-8 pr-3 py-1.5 text-sm bg-white border border-gray-200 rounded-lg outline-none focus:ring-2 focus:ring-gray-900/10 focus:border-gray-400 placeholder-gray-400 transition"
        />
      </div>
      <button
        onClick={onAddGame}
        className="px-3 py-1.5 text-sm font-semibold bg-gray-900 text-white rounded-lg hover:bg-gray-700 transition-colors whitespace-nowrap"
      >
        Add Game
      </button>
    </div>
  );
}
