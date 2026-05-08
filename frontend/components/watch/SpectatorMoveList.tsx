"use client";

import { useEffect, useMemo, useRef } from "react";
import { Chess } from "chess.js";
import type { SpectatorMove } from "@/hook/useSpectatorSocket";
import { cn } from "@/lib/utils";

interface SpectatorMoveListProps {
  moves: SpectatorMove[];
  currentFen: string;
}

export function SpectatorMoveList({ moves, currentFen }: SpectatorMoveListProps) {
  const endRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: "smooth", block: "end" });
  }, [moves]);

  const groupedMoves = useMemo(() => {
    const rows: Array<{ moveNumber: number; white?: SpectatorMove; black?: SpectatorMove }> = [];

    for (let index = 0; index < moves.length; index += 2) {
      rows.push({
        moveNumber: Math.floor(index / 2) + 1,
        white: moves[index],
        black: moves[index + 1],
      });
    }

    return rows;
  }, [moves]);

  const sideToMove = useMemo(() => {
    try {
      return new Chess(currentFen).turn() === "w" ? "White" : "Black";
    } catch {
      return "White";
    }
  }, [currentFen]);

  return (
    <div
      className="rounded-xl border border-gray-700/40 bg-gray-800/60 p-4"
      role="region"
      aria-label="Live move list"
    >
      <div className="mb-4 flex items-center justify-between gap-3">
        <div>
          <h3 className="text-lg font-semibold text-white">Moves</h3>
          <p className="text-sm text-gray-400">Auto-scrolling live notation</p>
        </div>
        <div className="rounded-full border border-gray-700/40 bg-gray-900/50 px-3 py-1 text-xs uppercase tracking-[0.18em] text-gray-400">
          {sideToMove} to move
        </div>
      </div>

      <div className="max-h-[420px] overflow-y-auto pr-2" aria-live="polite">
        <div className="grid grid-cols-[28px_1fr_1fr] gap-1">
          {groupedMoves.map((pair) => {
            const whiteIndex = (pair.moveNumber - 1) * 2;
            const blackIndex = whiteIndex + 1;
            const latestIndex = moves.length - 1;

            return (
              <>
                <div
                  key={`move-number-${pair.moveNumber}`}
                  className="py-2 text-xs font-semibold text-gray-500"
                >
                  {pair.moveNumber}.
                </div>
                <div
                  key={`white-${pair.moveNumber}`}
                  className={cn(
                    "rounded-lg px-2 py-2 text-sm text-gray-300",
                    latestIndex === whiteIndex && "bg-indigo-600 font-semibold text-white",
                  )}
                >
                  {pair.white?.san ?? "—"}
                </div>
                <div
                  key={`black-${pair.moveNumber}`}
                  className={cn(
                    "rounded-lg px-2 py-2 text-sm text-gray-300",
                    latestIndex === blackIndex && "bg-indigo-600 font-semibold text-white",
                  )}
                >
                  {pair.black?.san ?? "—"}
                </div>
              </>
            );
          })}
          <div ref={endRef} />
        </div>
      </div>

      <div className="mt-4 border-t border-gray-700/30 pt-3 text-sm text-gray-400">
        {moves.length} half-moves tracked
      </div>
    </div>
  );
}
