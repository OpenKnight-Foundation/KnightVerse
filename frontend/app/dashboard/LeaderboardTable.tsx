"use client";

import { useMemo, useState } from "react";

interface LeaderboardRow {
  rank: number;
  player: string;
  tier: "Grandmaster" | "Master" | "Diamond" | "Gold" | "Silver" | "Bronze";
  rating: number;
  winRate: number;
}

const TIER_COLORS: Record<LeaderboardRow["tier"], string> = {
  Grandmaster: "bg-gradient-to-r from-red-500 to-yellow-400 text-black",
  Master: "bg-purple-600 text-white",
  Diamond: "bg-cyan-500 text-black",
  Gold: "bg-yellow-500 text-black",
  Silver: "bg-gray-400 text-black",
  Bronze: "bg-amber-700 text-white",
};

export function LeaderboardTable({ rows }: { rows: LeaderboardRow[] }) {
  const [search, setSearch] = useState("");
  const filtered = useMemo(
    () => rows.filter((r) => r.player.toLowerCase().includes(search.toLowerCase())),
    [rows, search],
  );

  return (
    <div>
      <input
        placeholder="Search player..."
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        className="mb-3 w-full max-w-xs rounded-md border px-3 py-2 text-sm"
      />
      <table className="w-full text-sm">
        <thead className="sticky top-0 bg-background">
          <tr>
            <th className="p-2 text-left">Rank</th>
            <th className="p-2 text-left">Player</th>
            <th className="p-2 text-left">Tier</th>
            <th className="p-2 text-left">Rating</th>
            <th className="p-2 text-left">Win %</th>
          </tr>
        </thead>
        <tbody>
          {filtered.map((r) => (
            <tr key={r.rank} className="border-t">
              <td className="p-2">{r.rank}</td>
              <td className="p-2">{r.player}</td>
              <td className="p-2">
                <span className={`rounded px-2 py-0.5 text-xs ${TIER_COLORS[r.tier]}`}>{r.tier}</span>
              </td>
              <td className="p-2">{r.rating}</td>
              <td className="p-2">{r.winRate}%</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
