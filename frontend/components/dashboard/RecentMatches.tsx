"use client";

import type { EloDataPoint } from "@/components/profile/EloRatingChart";
import { cn } from "@/lib/utils";

interface RecentMatchesProps {
  data: EloDataPoint[];
}

function getResult(change: number): "W" | "L" | "D" {
  if (change > 0) {
    return "W";
  }

  if (change < 0) {
    return "L";
  }

  return "D";
}

function formatOpponent(opponent: string): string {
  if (opponent.length <= 12) {
    return opponent;
  }

  return `${opponent.slice(0, 6)}...${opponent.slice(-4)}`;
}

const badgeClassMap = {
  W: "border-emerald-500/30 bg-emerald-500/15 text-emerald-400",
  L: "border-red-500/30 bg-red-500/15 text-red-400",
  D: "border-yellow-500/30 bg-yellow-500/15 text-yellow-400",
} as const;

export default function RecentMatches({ data }: RecentMatchesProps) {
  const matches = [...data].slice(-10).reverse();

  return (
    <section
      role="region"
      aria-label="Recent matches"
      className="rounded-xl border border-gray-700/30 bg-gray-800/40 p-6 shadow-lg shadow-black/10"
    >
      <div className="mb-5 flex items-center justify-between gap-3">
        <div>
          <h2 className="text-lg font-semibold text-white">Recent matches</h2>
          <p className="mt-1 text-sm text-gray-400">Last 10 rating events in the selected range.</p>
        </div>
        <span className="rounded-full border border-gray-700/60 bg-gray-900/60 px-3 py-1 text-xs uppercase tracking-[0.18em] text-gray-400">
          {matches.length} shown
        </span>
      </div>

      <div className="overflow-x-auto">
        <table className="min-w-full text-left text-sm" aria-label="Recent ELO matches table">
          <thead>
            <tr className="border-b border-gray-700/40 text-xs uppercase tracking-[0.18em] text-gray-500">
              <th className="px-3 py-3 font-medium">Date</th>
              <th className="px-3 py-3 font-medium">Opponent</th>
              <th className="px-3 py-3 font-medium">Result</th>
              <th className="px-3 py-3 font-medium">Δ ELO</th>
              <th className="px-3 py-3 font-medium">New ELO</th>
            </tr>
          </thead>
          <tbody>
            {matches.map((match, index) => {
              const result = getResult(match.change);

              return (
                <tr
                  key={`${match.date}-${match.opponent}-${index}`}
                  className={cn(
                    "border-b border-gray-700/20 transition-colors hover:bg-gray-700/30",
                    index % 2 === 0 ? "bg-white/[0.015]" : "bg-transparent",
                  )}
                >
                  <td className="px-3 py-3 text-gray-300">
                    {new Date(match.date).toLocaleDateString("en-US", {
                      month: "short",
                      day: "numeric",
                    })}
                  </td>
                  <td className="px-3 py-3 text-white">{formatOpponent(match.opponent)}</td>
                  <td className="px-3 py-3">
                    <span className={cn("inline-flex min-w-10 items-center justify-center rounded-full border px-2.5 py-1 text-xs font-semibold", badgeClassMap[result])}>
                      {result}
                    </span>
                  </td>
                  <td className={cn("px-3 py-3 font-semibold", match.change >= 0 ? "text-emerald-400" : "text-red-400")}>
                    {match.change >= 0 ? "+" : ""}{match.change}
                  </td>
                  <td className="px-3 py-3 text-gray-200">{match.elo}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </section>
  );
}
