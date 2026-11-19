"use client";

import React, { useEffect, useState } from "react";
import {
  getPositionMoveStats,
  type MoveStat,
  type PositionMoveStats,
} from "@/lib/openingMoveStats";

interface OpeningMoveStatsProps {
  fen: string;
  /** Called with the SAN of a clicked move; return false if it couldn't be played. */
  onPlayMove: (san: string) => boolean | void;
  disabled?: boolean;
}

const pct = (n: number, total: number) => (total > 0 ? (n / total) * 100 : 0);

function formatGames(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return String(n);
}

function WdlBar({ stat }: { stat: MoveStat }) {
  const w = pct(stat.white, stat.games);
  const d = pct(stat.draws, stat.games);
  const b = pct(stat.black, stat.games);
  const label = `White ${w.toFixed(0)}%, draws ${d.toFixed(0)}%, black ${b.toFixed(0)}%`;
  return (
    <div
      className="flex h-4 w-full overflow-hidden rounded text-[10px] font-semibold leading-4"
      role="img"
      aria-label={label}
      title={label}
    >
      <div className="bg-gray-100 text-gray-900 text-center" style={{ width: `${w}%` }}>
        {w >= 12 ? `${w.toFixed(0)}%` : ""}
      </div>
      <div className="bg-gray-500 text-white text-center" style={{ width: `${d}%` }}>
        {d >= 12 ? `${d.toFixed(0)}%` : ""}
      </div>
      <div className="bg-gray-900 text-gray-100 text-center" style={{ width: `${b}%` }}>
        {b >= 12 ? `${b.toFixed(0)}%` : ""}
      </div>
    </div>
  );
}

export function OpeningMoveStats({ fen, onPlayMove, disabled }: OpeningMoveStatsProps) {
  const [data, setData] = useState<PositionMoveStats | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    getPositionMoveStats(fen, controller.signal).then((stats) => {
      if (controller.signal.aborted) return;
      setData(stats);
      setLoading(false);
    });
    return () => controller.abort();
  }, [fen]);

  const moves = data?.fen === fen ? data.moves : [];
  const total = moves.reduce((sum, m) => sum + m.games, 0);

  return (
    <div
      className="mt-4 rounded-xl border border-gray-700/50 bg-gray-900/50 p-3"
      role="region"
      aria-label="Move statistics"
    >
      <div className="flex items-center justify-between mb-2">
        <h3 className="text-sm font-semibold text-white">Move statistics</h3>
        {data?.source && data.source !== "none" && (
          <span className="text-[10px] text-gray-500">
            {data.source === "knightverse" ? "KnightVerse games" : "Masters database"}
          </span>
        )}
      </div>

      {loading && moves.length === 0 ? (
        <p className="text-xs text-gray-500" aria-live="polite">Loading statistics…</p>
      ) : moves.length === 0 ? (
        <p className="text-xs text-gray-500">No games found for this position.</p>
      ) : (
        <table className="w-full text-xs">
          <thead>
            <tr className="text-gray-500 text-left">
              <th className="font-medium pb-1 w-14">Move</th>
              <th className="font-medium pb-1 w-20 text-right pr-2">Games</th>
              <th className="font-medium pb-1">White / Draw / Black</th>
            </tr>
          </thead>
          <tbody>
            {moves.slice(0, 12).map((m) => (
              <tr key={m.san} className="hover:bg-gray-700/30">
                <td className="py-1">
                  <button
                    onClick={() => onPlayMove(m.san)}
                    disabled={disabled}
                    className="font-mono font-semibold text-teal-300 hover:text-teal-200 hover:underline disabled:opacity-50 disabled:cursor-not-allowed"
                    aria-label={`Play ${m.san}`}
                  >
                    {m.san}
                  </button>
                </td>
                <td className="py-1 text-right pr-2 text-gray-300 tabular-nums">
                  {formatGames(m.games)}
                  <span className="text-gray-500 ml-1">
                    {pct(m.games, total).toFixed(0)}%
                  </span>
                </td>
                <td className="py-1">
                  <WdlBar stat={m} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}

export default OpeningMoveStats;
