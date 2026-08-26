"use client";

import { Eye, Flame, Swords } from "lucide-react";
import { cn } from "@/lib/utils";
import { truncateAddress } from "@/lib/spectatorUtils";

interface LiveGameCardProps {
  gameId: string;
  white: { address: string; elo: number };
  black: { address: string; elo: number };
  moveCount: number;
  spectatorCount: number;
  timeControl: string;
  status: "playing" | "ending_soon";
  onWatch: (gameId: string) => void;
}

export function LiveGameCard({
  gameId,
  white,
  black,
  moveCount,
  spectatorCount,
  timeControl,
  status,
  onWatch,
}: LiveGameCardProps) {
  const isEndingSoon = status === "ending_soon";

  return (
    <button
      type="button"
      onClick={() => onWatch(gameId)}
      className="w-full rounded-xl border border-gray-700/30 bg-gray-800/40 p-5 text-left hover:border-teal-500/40 hover:bg-gray-800/60 transition-all duration-300 cursor-pointer animate-scale-in"
      aria-label={`Watch live game ${truncateAddress(white.address)} versus ${truncateAddress(black.address)}`}
    >
      <div className="flex items-start justify-between gap-3">
        <div className="inline-flex items-center rounded-full border border-blue-500/30 bg-blue-500/10 px-3 py-1 text-xs font-semibold text-blue-300">
          {timeControl}
        </div>
        <div
          className={cn(
            "inline-flex items-center gap-2 rounded-full border px-3 py-1 text-xs font-semibold",
            isEndingSoon
              ? "border-amber-500/30 bg-amber-500/15 text-amber-300"
              : "border-emerald-500/30 bg-emerald-500/15 text-emerald-400",
          )}
        >
          <span
            className={cn(
              "h-2 w-2 rounded-full",
              isEndingSoon ? "bg-amber-400" : "bg-emerald-400 animate-pulse-glow",
            )}
          />
          {isEndingSoon ? "Ending Soon" : "Live"}
        </div>
      </div>

      <div className="mt-5 space-y-3">
        <div className="flex items-center justify-between gap-3 rounded-xl border border-gray-700/30 bg-gray-900/40 px-4 py-3">
          <div>
            <p className="text-xs uppercase tracking-[0.18em] text-gray-500">White</p>
            <p className="mt-1 text-base font-semibold text-white">{truncateAddress(white.address)}</p>
          </div>
          <span className="rounded-full border border-gray-700/50 bg-gray-800/70 px-3 py-1 text-sm text-gray-300">
            {white.elo}
          </span>
        </div>
        <div className="flex items-center justify-between gap-3 rounded-xl border border-gray-700/30 bg-gray-900/40 px-4 py-3">
          <div>
            <p className="text-xs uppercase tracking-[0.18em] text-gray-500">Black</p>
            <p className="mt-1 text-base font-semibold text-white">{truncateAddress(black.address)}</p>
          </div>
          <span className="rounded-full border border-gray-700/50 bg-gray-800/70 px-3 py-1 text-sm text-gray-300">
            {black.elo}
          </span>
        </div>
      </div>

      <div className="mt-5 flex items-center justify-between gap-3">
        <div className="flex items-center gap-4 text-sm text-gray-400">
          <span className="inline-flex items-center gap-1.5">
            <Swords className="h-4 w-4" />
            {moveCount} moves
          </span>
          <span className="inline-flex items-center gap-1.5">
            <Eye className="h-4 w-4" />
            {spectatorCount}
          </span>
        </div>
        <span className="inline-flex items-center rounded-xl bg-gradient-to-r from-teal-500 to-blue-700 px-4 py-2 text-sm font-semibold text-white transition-all duration-300 hover:from-teal-600 hover:to-blue-800">
          <Flame className="mr-2 h-4 w-4" />
          Watch
        </span>
      </div>
    </button>
  );
}
