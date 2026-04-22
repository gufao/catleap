import type { Game } from "../types";
import { GameCard } from "./GameCard";

interface GameGridProps {
  games: Game[];
  onPlay: (gameId: string) => void;
  onStop: (gameId: string) => void;
}

export function GameGrid({ games, onPlay, onStop }: GameGridProps) {
  if (games.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-24 text-center">
        <div className="w-16 h-16 rounded-2xl bg-gray-100 flex items-center justify-center mb-4">
          <svg
            className="w-7 h-7 text-gray-400"
            fill="none"
            stroke="currentColor"
            strokeWidth={1.5}
            viewBox="0 0 24 24"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M15.75 10.5V6a3.75 3.75 0 1 0-7.5 0v4.5m11.356-1.993 1.263 12c.07.665-.45 1.243-1.119 1.243H4.25a1.125 1.125 0 0 1-1.12-1.243l1.264-12A1.125 1.125 0 0 1 5.513 7.5h12.974c.576 0 1.059.435 1.119 1.007Z"
            />
          </svg>
        </div>
        <p className="text-sm font-medium text-gray-500">No games found</p>
        <p className="text-xs text-gray-400 mt-1">
          Try scanning your Steam library or adding a game manually.
        </p>
      </div>
    );
  }

  return (
    <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 gap-4">
      {games.map((game) => (
        <GameCard key={game.id} game={game} onPlay={onPlay} onStop={onStop} />
      ))}
    </div>
  );
}
