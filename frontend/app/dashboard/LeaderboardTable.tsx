"use client";

import { useMemo, useState } from "react";
import Link from "next/link";
import { playerHref } from "@/lib/playerProfileService";

export type LeaderboardTier = "Grandmaster" | "Master" | "Diamond" | "Gold" | "Silver" | "Bronze";

export interface LeaderboardRow {
  rank: number;
  player: string;
  tier: LeaderboardTier;
  rating: number;
  winRate: number;
}

const TIER_COLORS: Record<LeaderboardTier, string> = {
  Grandmaster: "bg-gradient-to-r from-red-500 to-yellow-400 text-black",
  Master: "bg-purple-600 text-white",
  Diamond: "bg-cyan-500 text-black",
  Gold: "bg-yellow-500 text-black",
  Silver: "bg-gray-400 text-black",
  Bronze: "bg-amber-700 text-white",
};

export function LeaderboardTable({
  rows,
  highlightPlayer,
  searchable = true,
}: {
  rows: LeaderboardRow[];
  /** Username to highlight (the signed-in player). */
  highlightPlayer?: string | null;
  searchable?: boolean;
}) {
  const [search, setSearch] = useState("");
  const filtered = useMemo(
    () => rows.filter((r) => r.player.toLowerCase().includes(search.toLowerCase())),
    [rows, search],
  );

  return (
    <div>
      {searchable && (
        <input
          type="search"
          placeholder="Search player..."
          aria-label="Search player"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="mb-3 w-full max-w-xs rounded-xl border border-gray-700/50 bg-gray-900/70 px-3 py-2 text-sm text-white placeholder:text-gray-500"
        />
      )}
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <caption className="sr-only">Player leaderboard</caption>
          <thead className="sticky top-0 bg-gray-900/90 text-gray-400">
            <tr>
              <th scope="col" className="p-2 text-left">Rank</th>
              <th scope="col" className="p-2 text-left">Player</th>
              <th scope="col" className="p-2 text-left">Tier</th>
              <th scope="col" className="p-2 text-left">Rating</th>
              <th scope="col" className="p-2 text-left">Win %</th>
            </tr>
          </thead>
          <tbody>
            {filtered.length === 0 ? (
              <tr>
                <td colSpan={5} className="p-4 text-center text-gray-500">
                  No players found.
                </td>
              </tr>
            ) : (
              filtered.map((r) => {
                const isMe =
                  !!highlightPlayer && r.player.toLowerCase() === highlightPlayer.toLowerCase();
                return (
                  <tr
                    key={`${r.rank}-${r.player}`}
                    className={`border-t border-gray-700/40 ${isMe ? "bg-teal-500/10" : "hover:bg-gray-800/50"}`}
                    aria-current={isMe ? "true" : undefined}
                  >
                    <td className="p-2 font-semibold text-gray-300">{r.rank}</td>
                    <td className="p-2 text-white">
                      <Link href={playerHref(r.player)} className="hover:underline">
                        {r.player}
                      </Link>
                      {isMe && <span className="ml-2 text-xs text-teal-300">(you)</span>}
                    </td>
                    <td className="p-2">
                      <span className={`rounded px-2 py-0.5 text-xs ${TIER_COLORS[r.tier]}`}>{r.tier}</span>
                    </td>
                    <td className="p-2 tabular-nums text-gray-200">{r.rating}</td>
                    <td className="p-2 tabular-nums text-gray-200">{r.winRate}%</td>
                  </tr>
                );
              })
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}

export default LeaderboardTable;
