import type { Game } from "../types";
import { StatusBadge } from "./StatusBadge";

interface GameCardProps {
  game: Game;
  onPlay: (gameId: string) => void;
  onStop: (gameId: string) => void;
  onClick?: () => void;
}

function formatBytes(bytes: number): string {
  if (bytes >= 1_073_741_824) return `${(bytes / 1_073_741_824).toFixed(1)} GB`;
  if (bytes >= 1_048_576) return `${(bytes / 1_048_576).toFixed(0)} MB`;
  return `${bytes} B`;
}

// Deterministic gradient per game id for cover placeholder
function gradientForId(id: string): string {
  const hash = id.split("").reduce((acc, c) => acc + c.charCodeAt(0), 0);
  const gradients = [
    "from-violet-400 to-indigo-600",
    "from-sky-400 to-blue-600",
    "from-emerald-400 to-teal-600",
    "from-rose-400 to-pink-600",
    "from-amber-400 to-orange-600",
    "from-fuchsia-400 to-purple-600",
  ];
  return gradients[hash % gradients.length];
}

export function GameCard({ game, onPlay, onStop, onClick }: GameCardProps) {
  const isIncompatible = game.status === "incompatible";
  const subtitle = [
    game.source === "steam" ? "Steam" : "Manual",
    game.size_bytes ? formatBytes(game.size_bytes) : null,
  ]
    .filter(Boolean)
    .join(" · ");

  const playLabel =
    game.status === "unknown" ? "Try to Play" : "Play";

  return (
    <div
      onClick={onClick}
      className={`rounded-xl overflow-hidden bg-white shadow-sm border border-gray-100 flex flex-col transition-all duration-150 hover:shadow-md ${
        isIncompatible ? "opacity-60" : ""
      } ${onClick ? "cursor-pointer" : ""}`}
    >
      {/* Cover */}
      <div
        className={`h-32 bg-gradient-to-br ${gradientForId(game.id)} flex items-center justify-center`}
      >
        <span className="text-white/80 text-3xl font-bold select-none">
          {game.name.charAt(0).toUpperCase()}
        </span>
      </div>

      {/* Info */}
      <div className="p-3 flex flex-col gap-2 flex-1">
        <div className="flex items-start justify-between gap-1">
          <h3 className="text-sm font-semibold text-gray-900 leading-snug line-clamp-2">
            {game.name}
          </h3>
          <StatusBadge status={game.status} />
        </div>

        <p className="text-[11px] text-gray-400">{subtitle}</p>

        {/* Play / Stop button */}
        {!isIncompatible && (
          <div className="mt-auto pt-1">
            {game.is_running ? (
              <button
                onClick={(e) => { e.stopPropagation(); onStop(game.id); }}
                className="w-full flex items-center justify-center gap-1.5 px-3 py-1.5 rounded-lg bg-red-50 text-red-600 text-xs font-semibold hover:bg-red-100 transition-colors"
              >
                <span className="w-1.5 h-1.5 rounded-full bg-red-500 animate-pulse" />
                Playing
              </button>
            ) : (
              <button
                onClick={(e) => { e.stopPropagation(); onPlay(game.id); }}
                className="w-full px-3 py-1.5 rounded-lg bg-gray-900 text-white text-xs font-semibold hover:bg-gray-700 transition-colors"
              >
                {playLabel}
              </button>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
