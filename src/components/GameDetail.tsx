import { useState, useEffect } from "react";
import type { Game } from "../types";
import { StatusBadge } from "./StatusBadge";
import { readGameLog } from "../lib/tauri";

interface GameDetailProps {
  game: Game;
  onPlay: (gameId: string) => void;
  onStop: (gameId: string) => void;
  onBack: () => void;
}

function formatBytes(bytes: number): string {
  if (bytes >= 1_073_741_824) return `${(bytes / 1_073_741_824).toFixed(1)} GB`;
  if (bytes >= 1_048_576) return `${(bytes / 1_048_576).toFixed(0)} MB`;
  return `${bytes} B`;
}

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

export function GameDetail({ game, onPlay, onStop, onBack }: GameDetailProps) {
  const [logsExpanded, setLogsExpanded] = useState(false);
  const [logContent, setLogContent] = useState<string | null>(null);
  const [logLoading, setLogLoading] = useState(false);
  const isIncompatible = game.status === "incompatible";

  useEffect(() => {
    if (!logsExpanded) return;
    let cancelled = false;
    setLogLoading(true);
    readGameLog(game.id)
      .then((content) => { if (!cancelled) setLogContent(content); })
      .catch(() => { if (!cancelled) setLogContent(""); })
      .finally(() => { if (!cancelled) setLogLoading(false); });
    return () => { cancelled = true; };
  }, [logsExpanded, game.id]);

  const playLabel = game.status === "unknown" ? "Try to Play" : "Play";

  return (
    <div className="p-8 max-w-2xl">
      {/* Back */}
      <button
        onClick={onBack}
        className="flex items-center gap-1.5 text-sm text-gray-500 hover:text-gray-800 transition-colors mb-8"
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

      {/* Header */}
      <div className="flex items-start gap-5 mb-8">
        <div
          className={`w-24 h-24 rounded-2xl bg-gradient-to-br ${gradientForId(game.id)} flex items-center justify-center shrink-0`}
        >
          <span className="text-white/80 text-4xl font-bold select-none">
            {game.name.charAt(0).toUpperCase()}
          </span>
        </div>
        <div className="flex flex-col gap-2 min-w-0">
          <div className="flex items-center gap-2 flex-wrap">
            <h1 className="text-2xl font-semibold text-gray-900 leading-none">{game.name}</h1>
            <StatusBadge status={game.status} />
          </div>
          <div className="flex items-center gap-2 text-sm text-gray-400">
            <span>{game.source === "steam" ? "Steam" : "Manual"}</span>
            {game.size_bytes && (
              <>
                <span>·</span>
                <span>{formatBytes(game.size_bytes)}</span>
              </>
            )}
          </div>

          {/* Play / Stop */}
          {!isIncompatible && (
            <div className="mt-1">
              {game.is_running ? (
                <button
                  onClick={() => onStop(game.id)}
                  className="flex items-center gap-1.5 px-4 py-2 rounded-lg bg-red-50 text-red-600 text-sm font-semibold hover:bg-red-100 transition-colors"
                >
                  <span className="w-2 h-2 rounded-full bg-red-500 animate-pulse" />
                  Playing — Click to Stop
                </button>
              ) : (
                <button
                  onClick={() => onPlay(game.id)}
                  className="px-4 py-2 rounded-lg bg-gray-900 text-white text-sm font-semibold hover:bg-gray-700 transition-colors"
                >
                  {playLabel}
                </button>
              )}
            </div>
          )}
        </div>
      </div>

      {/* Notes / Compat */}
      {game.notes && (
        <section className="mb-6">
          <h2 className="text-xs font-semibold text-gray-400 uppercase tracking-wider mb-2">
            Compatibility Notes
          </h2>
          <p className="text-sm text-gray-700 bg-amber-50 border border-amber-100 rounded-lg px-4 py-3">
            {game.notes}
          </p>
        </section>
      )}

      {/* Info */}
      <section className="mb-6">
        <h2 className="text-xs font-semibold text-gray-400 uppercase tracking-wider mb-2">
          Details
        </h2>
        <div className="rounded-lg border border-gray-100 bg-white divide-y divide-gray-50">
          <div className="flex items-center justify-between px-4 py-2.5">
            <span className="text-sm text-gray-500">Source</span>
            <span className="text-sm font-medium text-gray-900">
              {game.source === "steam" ? "Steam" : "Manual"}
            </span>
          </div>
          <div className="flex items-center justify-between px-4 py-2.5">
            <span className="text-sm text-gray-500">Status</span>
            <StatusBadge status={game.status} />
          </div>
          {game.size_bytes && (
            <div className="flex items-center justify-between px-4 py-2.5">
              <span className="text-sm text-gray-500">Size</span>
              <span className="text-sm font-medium text-gray-900">
                {formatBytes(game.size_bytes)}
              </span>
            </div>
          )}
          {game.executable && (
            <div className="flex items-start justify-between px-4 py-2.5 gap-4">
              <span className="text-sm text-gray-500 shrink-0">Executable</span>
              <span className="text-sm font-medium text-gray-900 text-right break-all">
                {game.executable}
              </span>
            </div>
          )}
        </div>
      </section>

      {/* Logs */}
      <section>
        <button
          onClick={() => setLogsExpanded((v) => !v)}
          className="flex items-center gap-2 text-xs font-semibold text-gray-400 uppercase tracking-wider mb-2 hover:text-gray-600 transition-colors"
        >
          <svg
            className={`w-3.5 h-3.5 transition-transform ${logsExpanded ? "rotate-90" : ""}`}
            fill="none"
            stroke="currentColor"
            strokeWidth={2}
            viewBox="0 0 24 24"
          >
            <path strokeLinecap="round" strokeLinejoin="round" d="m9 18 6-6-6-6" />
          </svg>
          Launch Logs
        </button>

        {logsExpanded && (
          <div className="rounded-lg border border-gray-100 bg-gray-950 overflow-hidden">
            {logLoading ? (
              <div className="px-4 py-3 text-sm text-gray-400">Loading logs…</div>
            ) : logContent ? (
              <pre className="px-4 py-3 text-xs text-green-400 font-mono max-h-64 overflow-auto whitespace-pre-wrap">
                <code>{logContent}</code>
              </pre>
            ) : (
              <div className="px-4 py-3 text-sm text-gray-500">No logs available yet. Launch the game to generate logs.</div>
            )}
          </div>
        )}
      </section>
    </div>
  );
}
