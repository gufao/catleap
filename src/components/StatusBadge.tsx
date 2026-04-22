import type { GameStatus } from "../types";

interface StatusBadgeProps {
  status: GameStatus;
}

const statusConfig: Record<GameStatus, { label: string; className: string }> = {
  compatible: {
    label: "Compatible",
    className: "bg-green-100 text-green-700",
  },
  experimental: {
    label: "Experimental",
    className: "bg-amber-100 text-amber-700",
  },
  incompatible: {
    label: "Incompatible",
    className: "bg-red-100 text-red-700",
  },
  unknown: {
    label: "Unknown",
    className: "bg-gray-100 text-gray-500",
  },
};

export function StatusBadge({ status }: StatusBadgeProps) {
  const { label, className } = statusConfig[status];
  return (
    <span
      className={`inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-semibold uppercase tracking-wide ${className}`}
    >
      {label}
    </span>
  );
}
