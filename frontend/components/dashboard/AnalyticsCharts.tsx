"use client";

import { useMemo } from "react";
import { BarChart3, PieChart as PieChartIcon, Radar, Sparkles } from "lucide-react";
import {
  Bar,
  BarChart,
  CartesianGrid,
  Cell,
  Pie,
  PieChart,
  PolarAngleAxis,
  PolarGrid,
  // Aliased because the lucide-react `Radar` icon above already uses that name.
  Radar as RechartsRadar,
  RadarChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type { EloDataPoint } from "@/components/profile/EloRatingChart";
import { EmptyState } from "@/components/ui/empty-state";
import { DEFAULT_OPENINGS, buildColorWinBreakdown, buildOpeningWinRates, formatRatingDelta } from "@/lib/eloStatsUtils";

interface AnalyticsChartsProps {
  /** One entry per played game, in chronological order. */
  data: EloDataPoint[];
  /** The time range currently selected by the parent; shown as a badge only. */
  range: "7d" | "30d" | "90d" | "1y" | "all";
}

/**
 * Shape of the data Recharts passes to the custom tooltip. Every field in
 * `payload` is optional because a tooltip can fire for different charts
 * (rating bar, opening radar) that carry different fields.
 */
interface ChartTooltipEntry {
  name: string;
  value: number;
  payload?: {
    date?: string;
    opponent?: string;
    change?: number;
    result?: string;
    elo?: number;
    opening?: string;
    winRate?: number;
  };
}

// Slice colors for the pie chart, applied by index: green, red, yellow.
// The order matters: it must match the order of `colorBreakdown` entries.
const RESULT_COLORS = ["#34d399", "#f87171", "#facc15"];

/** Converts a rating change into a result label: gain = win, loss = loss, zero = draw. */
function getResultLabel(change: number): string {
  if (change > 0) return "Win";
  if (change < 0) return "Loss";
  return "Draw";
}

/** Formats an ISO date string as e.g. "Mar 5, 2025" for the tooltip heading. */
function formatDate(date: string): string {
  return new Date(date).toLocaleDateString("en-US", {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

/**
 * Dashboard of three player-analytics charts:
 *  1. Rating history (bar chart of rating after each game)
 *  2. Color split (donut chart of games per color, with win rates)
 *  3. Opening repertoire (radar chart plus progress bars of win rates)
 *
 * Shows an empty state when there is no game data.
 */
export default function AnalyticsCharts({ data, range }: AnalyticsChartsProps) {
  // Enrich each game with a human-readable result and formatted delta.
  // Memoized so the mapping only reruns when `data` changes, not on every render.
  const ratingHistory = useMemo(() => {
    return data.map((point) => ({
      date: point.date,
      elo: point.elo,
      change: point.change,
      opponent: point.opponent,
      result: getResultLabel(point.change),
      formattedDelta: formatRatingDelta(point.change),
    }));
  }, [data]);

  // Aggregations for the pie and radar charts, computed by helpers in
  // `eloStatsUtils`. Memoized for the same reason as above.
  const colorBreakdown = useMemo(() => buildColorWinBreakdown(data), [data]);
  const openingBreakdown = useMemo(() => buildOpeningWinRates(data), [data]);

  // Early return: with no games there is nothing to chart, so show a
  // placeholder card spanning both grid columns instead of empty charts.
  if (!data.length) {
    return (
      <section
        role="region"
        aria-label="Player analytics charts"
        className="grid gap-6 xl:grid-cols-2"
      >
        <div className="rounded-2xl border border-dashed border-gray-700 bg-gray-900/40 xl:col-span-2">
          <EmptyState
            icon={<Sparkles className="h-12 w-12 text-teal-300" />}
            title="No analytics yet"
            description="Play at least 5 games to unlock rating progression, color split, and repertoire insights."
          />
        </div>
      </section>
    );
  }

  return (
    // Two-column grid on extra-large screens, single column below that.
    // `role` and `aria-label` make the dashboard a labelled landmark for screen readers.
    <section
      role="region"
      aria-label="Player analytics charts"
      className="grid gap-6 xl:grid-cols-2"
    >
      {/* ── Card 1: Rating history ───────────────────────────────────── */}
      <div className="rounded-2xl border border-gray-700/30 bg-gray-900/50 p-4 md:p-6">
        <div className="mb-4 flex items-center justify-between gap-3">
          <div className="flex items-center gap-2 text-sm font-semibold text-white">
            <BarChart3 className="h-4 w-4 text-teal-400" />
            Rating history
          </div>
          {/* Badge showing which time range the data covers */}
          <span className="rounded-full border border-gray-700/50 bg-gray-800/70 px-2.5 py-1 text-[10px] uppercase tracking-[0.14em] text-gray-400">
            {range}
          </span>
        </div>
        {/* ResponsiveContainer needs a parent with an explicit height (h-72) to size the chart */}
        <div className="h-72 w-full">
          <ResponsiveContainer width="100%" height="100%">
            <BarChart data={ratingHistory} margin={{ top: 10, right: 10, left: 0, bottom: 0 }}>
              {/* Horizontal grid lines only, kept faint so bars stay the focus */}
              <CartesianGrid stroke="rgba(148,163,184,0.12)" vertical={false} />
              <XAxis
                dataKey="date"
                // Short "Mar 5" labels on the axis; the full date appears in the tooltip
                tickFormatter={(value) => new Date(value).toLocaleDateString("en-US", { month: "short", day: "numeric" })}
                tick={{ fill: "#94a3b8", fontSize: 11 }}
                axisLine={false}
                tickLine={false}
                // Skip labels when they would be closer than 18px, to avoid overlap
                minTickGap={18}
              />
              <YAxis tick={{ fill: "#94a3b8", fontSize: 11 }} axisLine={false} tickLine={false} width={40} />
              {/* Custom tooltip showing date, opponent, result and rating change */}
              <Tooltip
                cursor={{ fill: "rgba(148,163,184,0.08)" }}
                content={({ active, payload }) => {
                  // Render nothing unless the user is hovering over a bar
                  if (!active || !payload?.length) return null;
                  // The hovered game's data is on the first payload entry
                  const point = payload[0]?.payload as ChartTooltipEntry["payload"] | undefined;
                  if (!point) return null;

                  return (
                    <div className="rounded-xl border border-gray-700 bg-slate-900/95 px-3 py-2 text-sm text-gray-200 shadow-2xl shadow-black/20">
                      <p className="font-semibold text-white">{point.date ? formatDate(point.date) : "Match"}</p>
                      <p className="mt-1 text-xs text-gray-400">vs {point.opponent ?? "Unknown"}</p>
                      <p className="mt-2 text-xs text-gray-300">
                        Result: <span className="font-medium text-white">{point.result ?? "—"}</span>
                      </p>
                      {/* Green for gains and draws (>= 0), red for losses */}
                      <p className={`mt-1 text-xs font-semibold ${point.change !== undefined && point.change >= 0 ? "text-emerald-400" : "text-red-400"}`}>
                        Rating delta: {point.change !== undefined ? formatRatingDelta(point.change) : "0"}
                      </p>
                    </div>
                  );
                }}
              />
              {/* One bar per game, height = rating after that game; rounded top corners only */}
              <Bar dataKey="elo" radius={[8, 8, 0, 0]} fill="#2dd4bf" />
            </BarChart>
          </ResponsiveContainer>
        </div>
      </div>

      {/* ── Card 2: Color split (donut chart plus legend) ────────────── */}
      <div className="rounded-2xl border border-gray-700/30 bg-gray-900/50 p-4 md:p-6">
        <div className="mb-4 flex items-center gap-2 text-sm font-semibold text-white">
          <PieChartIcon className="h-4 w-4 text-emerald-400" />
          Color split
        </div>
        <div className="h-72 w-full">
          <ResponsiveContainer width="100%" height="100%">
            <PieChart>
              {/* A nonzero innerRadius turns the pie into a donut; slices are sized by game count */}
              <Pie data={colorBreakdown} dataKey="games" innerRadius={48} outerRadius={74} paddingAngle={3} stroke="none">
                {/* Color each slice from RESULT_COLORS by its position */}
                {colorBreakdown.map((entry, index) => (
                  <Cell key={entry.label} fill={RESULT_COLORS[index]} />
                ))}
              </Pie>
              <Tooltip
                formatter={(value) => {
                  // Recharts may pass the value as an array; normalize to a single number
                  const numeric = Number(Array.isArray(value) ? value[0] : value ?? 0);
                  return [`${numeric} games`, "Games"];
                }}
                contentStyle={{ backgroundColor: "#0f172a", border: "1px solid rgba(148,163,184,0.2)", borderRadius: 12 }}
              />
            </PieChart>
          </ResponsiveContainer>
        </div>
        {/* Legend below the chart: color dot, label, win rate and game count */}
        <div className="mt-2 space-y-2">
          {colorBreakdown.map((entry, index) => (
            <div key={entry.label} className="flex items-center justify-between gap-3 text-sm text-gray-300">
              <div className="flex items-center gap-2">
                <span className="h-2.5 w-2.5 rounded-full" style={{ backgroundColor: RESULT_COLORS[index] }} />
                {entry.label}
              </div>
              <div className="text-right">
                <span className="font-semibold text-white">{entry.winRate.toFixed(0)}%</span>
                <span className="ml-2 text-gray-500">{entry.games} games</span>
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* ── Card 3: Opening repertoire (spans both columns) ──────────── */}
      <div className="rounded-2xl border border-gray-700/30 bg-gray-900/50 p-4 md:p-6 xl:col-span-2">
        <div className="mb-4 flex items-center gap-2 text-sm font-semibold text-white">
          <Radar className="h-4 w-4 text-violet-400" />
          Opening repertoire win-rates
        </div>
        {/* Wide radar chart on the left, per-opening progress bars on the right (large screens) */}
        <div className="grid gap-6 lg:grid-cols-[1.2fr_0.8fr]">
          <div className="h-80 w-full">
            <ResponsiveContainer width="100%" height="100%">
              {/* If no opening stats exist yet, fall back to the default opening
                  names with zeroed values so the radar grid still renders */}
              <RadarChart data={openingBreakdown.length ? openingBreakdown : DEFAULT_OPENINGS.map((opening) => ({ opening, games: 0, wins: 0, winRate: 0 }))}>
                <PolarGrid stroke="rgba(148,163,184,0.2)" />
                {/* Each axis around the radar is one opening */}
                <PolarAngleAxis dataKey="opening" tick={{ fill: "#cbd5e1", fontSize: 11 }} />
                {/* The filled shape plots win rate (0-100%) per opening */}
                <RechartsRadar dataKey="winRate" stroke="#a78bfa" fill="#8b5cf6" fillOpacity={0.4} />
                <Tooltip
                  formatter={(value) => {
                    // Normalize array-or-scalar values, then show as a whole-number percentage
                    const numeric = Number(Array.isArray(value) ? value[0] : value ?? 0);
                    return [`${numeric.toFixed(0)}%`, "Win rate"];
                  }}
                  contentStyle={{ backgroundColor: "#0f172a", border: "1px solid rgba(148,163,184,0.2)", borderRadius: 12 }}
                />
              </RadarChart>
            </ResponsiveContainer>
          </div>

          {/* Textual breakdown: one progress bar per opening, width = win rate */}
          <div className="space-y-3">
            {openingBreakdown.map((entry) => (
              <div key={entry.opening} className="rounded-xl border border-gray-700/30 bg-gray-800/70 p-3">
                <div className="mb-2 flex items-center justify-between gap-3 text-sm text-gray-200">
                  <span className="font-medium text-white">{entry.opening}</span>
                  <span className="font-semibold text-violet-300">{entry.winRate.toFixed(0)}%</span>
                </div>
                {/* Track (gray) with a gradient fill whose width is the win-rate percentage */}
                <div className="h-2 overflow-hidden rounded-full bg-gray-700/60">
                  <div className="h-full rounded-full bg-gradient-to-r from-violet-400 to-fuchsia-500" style={{ width: `${entry.winRate}%` }} />
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </section>
  );
}