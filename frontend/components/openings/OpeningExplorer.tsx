/**
 * OpeningExplorer — interactive ECO tree + W/D/L stats (#1112 FE-53).
 */
"use client";

import React from "react";
import {
  ECO_BOOK,
  classifyOpening,
  winRateBar,
  type OpeningStats,
} from "@/lib/openings";

interface Props {
  selectedEco: string | null;
  onSelect: (eco: string) => void;
  statsByEco: Record<string, OpeningStats>;
  depthByEco: Record<string, number>;
  frequencyByEco: Record<string, number>;
}

export function OpeningExplorer({
  selectedEco,
  onSelect,
  statsByEco,
  depthByEco,
  frequencyByEco,
}: Props) {
  return (
    <div
      className="bg-gray-800/60 rounded-xl border border-gray-700/50 p-4"
      role="region"
      aria-label="Opening explorer"
    >
      <h2 className="text-lg font-bold text-white mb-1">Opening Explorer</h2>
      <p className="text-xs text-gray-400 mb-4">
        ECO codes with master win/draw/loss rates. Select a line to drill it.
      </p>
      <ul className="space-y-2 max-h-[520px] overflow-y-auto pr-1">
        {ECO_BOOK.map((opening) => {
          const stats = statsByEco[opening.eco];
          const bar = stats ? winRateBar(stats) : null;
          const active = selectedEco === opening.eco;
          const depth = depthByEco[opening.eco] ?? opening.moves.length;
          const freq = frequencyByEco[opening.eco] ?? 0;
          return (
            <li key={opening.eco}>
              <button
                onClick={() => onSelect(opening.eco)}
                aria-pressed={active}
                aria-label={`${opening.eco} ${opening.name}`}
                className={`w-full text-left p-3 rounded-xl border transition-all hover:scale-[1.01] ${
                  active
                    ? "border-emerald-500/50 bg-emerald-500/10"
                    : "border-gray-700/50 bg-gray-900/60 hover:border-gray-600/60"
                }`}
              >
                <div className="flex items-center justify-between gap-2">
                  <span className="text-xs font-mono px-2 py-0.5 rounded bg-teal-500/15 text-teal-300 border border-teal-500/30">
                    {opening.eco}
                  </span>
                  <span className="text-[11px] text-gray-400">
                    depth {depth} · played {freq}x
                  </span>
                </div>
                <p className="text-sm font-semibold text-white mt-2">
                  {opening.name}
                </p>
                <p className="text-xs text-gray-400 truncate">
                  {opening.moves.join(" ")}
                </p>
                {stats && bar && (
                  <div className="mt-2">
                    <div
                      className="flex h-2 rounded-full overflow-hidden bg-gray-700"
                      role="img"
                      aria-label={`White ${bar.whitePct} percent, draws ${bar.drawPct} percent, Black ${bar.blackPct} percent`}
                    >
                      <div
                        className="bg-gray-100"
                        style={{ width: `${bar.whitePct}%` }}
                      />
                      <div
                        className="bg-gray-500"
                        style={{ width: `${bar.drawPct}%` }}
                      />
                      <div
                        className="bg-emerald-500"
                        style={{ width: `${bar.blackPct}%` }}
                      />
                    </div>
                    <p className="text-[11px] text-gray-400 mt-1">
                      W {bar.whitePct}% · D {bar.drawPct}% · B {bar.blackPct}% ·{" "}
                      {stats.games.toLocaleString()} games
                    </p>
                  </div>
                )}
              </button>
            </li>
          );
        })}
      </ul>
      <p className="text-[11px] text-gray-500 mt-3">
        Tip: your current board line classifies as{" "}
        <span className="text-gray-300">
          {classifyOpening([])?.name ?? "—"}
        </span>
        . Play moves on the board to see it update.
      </p>
    </div>
  );
}
