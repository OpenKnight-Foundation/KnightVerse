"use client";

import { Clock3, Trophy } from "lucide-react";
import { cn } from "@/lib/utils";
import {
  formatClock,
  getCapturedPieceSymbols,
  truncateAddress,
} from "@/lib/spectatorUtils";

interface PlayerPanelProps {
  address: string;
  elo: number;
  timeRemaining: number;
  isActive: boolean;
  color: "white" | "black";
  capturedPieces: string[];
}

export function PlayerPanel({
  address,
  elo,
  timeRemaining,
  isActive,
  color,
  capturedPieces,
}: PlayerPanelProps) {
  const capturedSymbols = getCapturedPieceSymbols(capturedPieces, color);
  const lowTime = timeRemaining < 30;

  return (
    <div
      className={cn(
        "rounded-xl border bg-gray-800/60 p-4 transition-all duration-300",
        isActive
          ? "border-teal-500/40 shadow-[0_0_24px_rgba(20,184,166,0.12)]"
          : "border-gray-700/40",
      )}
      role="region"
      aria-label={`${color} player panel`}
    >
      <div className="flex items-center justify-between gap-4">
        <div className="flex min-w-0 items-center gap-3">
          <div className="flex h-11 w-11 items-center justify-center rounded-full bg-gradient-to-br from-teal-400 to-blue-600 text-sm font-bold text-white">
            {address.slice(0, 2).toUpperCase()}
          </div>
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <p className="truncate text-sm font-semibold text-white">{truncateAddress(address)}</p>
              <span className="inline-flex items-center gap-1 rounded-full border border-gray-700/50 bg-gray-900/60 px-2 py-0.5 text-xs text-gray-300">
                <Trophy className="h-3 w-3 text-amber-400" />
                {elo}
              </span>
            </div>
            <p className="mt-1 text-xs uppercase tracking-[0.18em] text-gray-500">{color}</p>
          </div>
        </div>

        <div
          className={cn(
            "inline-flex items-center gap-2 rounded-xl border px-3 py-2 text-sm font-semibold",
            lowTime
              ? "border-red-500/30 bg-red-500/15 text-red-400 animate-pulse-glow"
              : "border-gray-700/50 bg-gray-900/60 text-white",
          )}
          aria-label={`${color} clock ${formatClock(timeRemaining)}`}
        >
          <Clock3 className="h-4 w-4" />
          {formatClock(timeRemaining)}
        </div>
      </div>

      <div className="mt-3 flex items-center gap-3 border-t border-gray-700/30 pt-3">
        <span className="text-xs uppercase tracking-[0.18em] text-gray-500">Captured</span>
        <div className="flex min-h-6 flex-wrap items-center gap-1 text-lg text-gray-200">
          {capturedSymbols.length > 0 ? (
            capturedSymbols.map((piece, index) => <span key={`${piece}-${index}`}>{piece}</span>)
          ) : (
            <span className="text-xs text-gray-500">No captures yet</span>
          )}
        </div>
      </div>
    </div>
  );
}
