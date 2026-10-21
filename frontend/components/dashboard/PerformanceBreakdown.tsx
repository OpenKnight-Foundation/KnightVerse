"use client";

import { useMemo } from "react";
import { Flame, Swords, Trophy } from "lucide-react";
import { Cell, Pie, PieChart, ResponsiveContainer } from "recharts";
import type { EloDataPoint } from "@/components/profile/EloRatingChart";
import { cn } from "@/lib/utils";

interface PerformanceBreakdownProps {
  /** One entry per game, in chronological order; used for the color and opening stats. */
  data: EloDataPoint[];
  wins: number;
  losses: number;
  draws: number;
  /** Positive = consecutive wins, negative = consecutive losses, 0 = no streak. */
  currentStreak: number;
  /** Longest winning streak in the sample. */
  bestStreak: number;
}

// Slice colors for the win/loss/draw ring, applied by index: green, red, yellow.
// The order must match the order of the `distribution` array below.
const RESULT_COLORS = ["#34d399", "#f87171", "#facc15"];

// Placeholder opening names. Games are assigned to these in rotation (see
// `openingBreakdown`), since the game data has no real opening information.
const OPENINGS = [
  "Sicilian Defense",
  "Queen's Gambit",
  "Ruy Lopez",
  "French Defense",
  "King's Indian",
] as const;

/**
 * Panel summarizing a player's results: a win/loss/draw ring, streak cards,
 * win rate by color, and win rate by opening.
 *
 * Note: the color and opening figures are mock data derived from game order,
 * not real game attributes (see the comments in the memos below).
 */
export default function PerformanceBreakdown({
  data,
  wins,
  losses,
  draws,
  currentStreak,
  bestStreak,
}: PerformanceBreakdownProps) {
  // Data for the donut chart and its legend: one slice per result type
  const distribution = [
    { name: "Wins", value: wins },
    { name: "Losses", value: losses },
    { name: "Draws", value: draws },
  ];
  const totalGames = wins + losses + draws;

  // Win rate per color. The data has no color field, so this assumes games
  // alternate White/Black starting with White (even index = White, odd = Black).
  const colorBreakdown = useMemo(() => {
    const buckets = {
      white: { label: "White", wins: 0, games: 0 },
      black: { label: "Black", wins: 0, games: 0 },
    };

    data.forEach((point, index) => {
      // Alternate between the two buckets by position
      const bucket = index % 2 === 0 ? buckets.white : buckets.black;
      bucket.games += 1;
      // A positive rating change is counted as a win
      if (point.change > 0) {
        bucket.wins += 1;
      }
    });

    // Add a percentage win rate, guarding against division by zero
    return Object.values(buckets).map((bucket) => ({
      ...bucket,
      winRate: bucket.games ? (bucket.wins / bucket.games) * 100 : 0,
    }));
  }, [data]);

  // Win rate per opening. Like the color split, this is deterministic mock
  // data: games are spread across OPENINGS round-robin by index.
  const openingBreakdown = useMemo(() => {
    // Start every opening at zero so each one appears even with no games
    const buckets = OPENINGS.reduce<Record<string, { games: number; wins: number }>>((acc, opening) => {
      acc[opening] = { games: 0, wins: 0 };
      return acc;
    }, {});

    data.forEach((point, index) => {
      // Cycle through the openings: game 0 -> first, game 1 -> second, ... wraps around
      const opening = OPENINGS[index % OPENINGS.length];
      buckets[opening].games += 1;
      // A positive rating change is counted as a win
      if (point.change > 0) {
        buckets[opening].wins += 1;
      }
    });

    return Object.entries(buckets)
      .map(([opening, bucket]) => ({
        opening,
        games: bucket.games,
        // Guard against division by zero for openings with no games
        winRate: bucket.games ? (bucket.wins / bucket.games) * 100 : 0,
      }))
      // Best win rate first
      .sort((left, right) => right.winRate - left.winRate);
  }, [data]);

  return (
    <section
      role="region"
      aria-label="Performance breakdown"
      className="rounded-xl border border-gray-700/30 bg-gray-800/40 p-6 shadow-lg shadow-black/10"
    >
      {/* Section heading */}
      <div className="mb-6">
        <h2 className="text-lg font-semibold text-white">Performance breakdown</h2>
        <p className="mt-1 text-sm text-gray-400">Win distribution, streak momentum, color edge, and opening comfort.</p>
      </div>

      {/* Two-column layout on extra-large screens: left = ring and streaks,
          right = color and opening stats. Stacks on smaller screens. */}
      <div className="grid gap-6 xl:grid-cols-[1.1fr_0.9fr]">
        <div className="grid gap-6 md:grid-cols-2">
          {/* ── Win/loss/draw ring ──────────────────────────────────── */}
          <div className="rounded-2xl border border-gray-700/40 bg-gray-900/50 p-4">
            <div className="mb-4 flex items-center gap-2 text-sm font-semibold text-white">
              <Trophy className="h-4 w-4 text-emerald-400" />
              Win rate ring
            </div>
            <div className="flex items-center gap-4">
              {/* Fixed-size box: ResponsiveContainer needs a parent with explicit dimensions */}
              <div className="h-40 w-40">
                <ResponsiveContainer width="100%" height="100%">
                  <PieChart>
                    {/* innerRadius > 0 makes this a donut */}
                    <Pie
                      data={distribution}
                      dataKey="value"
                      innerRadius={42}
                      outerRadius={60}
                      paddingAngle={4}
                      stroke="none"
                    >
                      {/* Color each slice from RESULT_COLORS by position */}
                      {distribution.map((entry, index) => (
                        <Cell key={entry.name} fill={RESULT_COLORS[index]} />
                      ))}
                    </Pie>
                  </PieChart>
                </ResponsiveContainer>
              </div>
              {/* Legend: color dot, label, count and percentage of total games */}
              <div className="space-y-3">
                {distribution.map((entry, index) => (
                  <div key={entry.name} className="flex items-center gap-3 text-sm text-gray-300">
                    <span className="h-2.5 w-2.5 rounded-full" style={{ backgroundColor: RESULT_COLORS[index] }} />
                    <span className="w-16">{entry.name}</span>
                    <span className="font-semibold text-white">{entry.value}</span>
                    <span className="text-gray-500">
                      {/* Avoid dividing by zero when no games have been played */}
                      {totalGames ? `${((entry.value / totalGames) * 100).toFixed(0)}%` : "0%"}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          </div>

          {/* ── Streak cards ────────────────────────────────────────── */}
          <div className="rounded-2xl border border-gray-700/40 bg-gray-900/50 p-4">
            <div className="mb-4 flex items-center gap-2 text-sm font-semibold text-white">
              <Flame className="h-4 w-4 text-orange-400" />
              Streak info
            </div>
            <div className="grid gap-3">
              {/* Current streak: green for wins, red for losses, gray for none */}
              <div className="rounded-xl border border-gray-700/40 bg-gray-800/70 p-4">
                <p className="text-xs uppercase tracking-[0.18em] text-gray-500">Current streak</p>
                {/* `cn` merges the class strings; the color depends on the streak's sign */}
                <p className={cn(
                  "mt-2 text-2xl font-bold",
                  currentStreak > 0 ? "text-emerald-400" : currentStreak < 0 ? "text-red-400" : "text-gray-300",
                )}>
                  {/* Show an explicit "+" for win streaks; negatives already carry "-" */}
                  {currentStreak > 0 ? `+${currentStreak}` : currentStreak}
                </p>
                <p className="mt-1 text-sm text-gray-400">
                  {currentStreak > 0 ? "Winning run active" : currentStreak < 0 ? "Pressure streak" : "No active streak"}
                </p>
              </div>
              {/* Best streak: always shown in neutral white */}
              <div className="rounded-xl border border-gray-700/40 bg-gray-800/70 p-4">
                <p className="text-xs uppercase tracking-[0.18em] text-gray-500">Best streak</p>
                <p className="mt-2 text-2xl font-bold text-white">{bestStreak}</p>
                <p className="mt-1 text-sm text-gray-400">Longest win streak in this sample.</p>
              </div>
            </div>
          </div>
        </div>

        <div className="grid gap-6">
          {/* ── Win rate by color ───────────────────────────────────── */}
          <div className="rounded-2xl border border-gray-700/40 bg-gray-900/50 p-4">
            <div className="mb-4 flex items-center gap-2 text-sm font-semibold text-white">
              <Swords className="h-4 w-4 text-cyan-400" />
              Win rate by color
            </div>
            <div className="space-y-3">
              {colorBreakdown.map((entry) => (
                <div key={entry.label} className="rounded-xl border border-gray-700/30 bg-gray-800/70 p-3">
                  <div className="mb-2 flex items-center justify-between text-sm text-white">
                    <span>{entry.label}</span>
                    <span>{entry.winRate.toFixed(0)}%</span>
                  </div>
                  {/* Progress bar: gray track with a gradient fill whose width is the win rate */}
                  <div className="h-2 overflow-hidden rounded-full bg-gray-700/60">
                    <div className="h-full rounded-full bg-gradient-to-r from-teal-400 to-blue-500" style={{ width: `${entry.winRate}%` }} />
                  </div>
                </div>
              ))}
            </div>
          </div>

          {/* ── Win rate by opening ─────────────────────────────────── */}
          <div className="rounded-2xl border border-gray-700/40 bg-gray-900/50 p-4">
            <div className="mb-4 flex items-center justify-between gap-3">
              <div>
                <p className="text-sm font-semibold text-white">Win rate by opening</p>
                <p className="mt-1 text-xs text-gray-500">Top lines based on deterministic mock repertoire.</p>
              </div>
              {/* Badge showing how many openings are listed */}
              <span className="rounded-full border border-gray-700/50 bg-gray-800/70 px-3 py-1 text-xs uppercase tracking-[0.18em] text-gray-400">
                {openingBreakdown.length} lines
              </span>
            </div>
            {/* Rows are already sorted from best to worst win rate */}
            <div className="space-y-3">
              {openingBreakdown.map((entry) => (
                <div key={entry.opening} className="flex items-center justify-between gap-4 rounded-xl border border-gray-700/30 bg-gray-800/70 px-3 py-3 text-sm">
                  <div>
                    <p className="font-medium text-white">{entry.opening}</p>
                    <p className="text-xs text-gray-500">{entry.games} games sampled</p>
                  </div>
                  <div className="text-right">
                    <p className="font-semibold text-teal-300">{entry.winRate.toFixed(0)}%</p>
                    <p className="text-xs text-gray-500">win rate</p>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}