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
  Radar as RechartsRadar,
  RadarChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type { EloDataPoint } from "@/components/profile/EloRatingChart";
import { DEFAULT_OPENINGS, buildColorWinBreakdown, buildOpeningWinRates, formatRatingDelta } from "@/lib/eloStatsUtils";

interface AnalyticsChartsProps {
  data: EloDataPoint[];
  range: "7d" | "30d" | "90d" | "1y" | "all";
}

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

const RESULT_COLORS = ["#34d399", "#f87171", "#facc15"];

function getResultLabel(change: number): string {
  if (change > 0) return "Win";
  if (change < 0) return "Loss";
  return "Draw";
}

function formatDate(date: string): string {
  return new Date(date).toLocaleDateString("en-US", {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

export default function AnalyticsCharts({ data, range }: AnalyticsChartsProps) {
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

  const colorBreakdown = useMemo(() => buildColorWinBreakdown(data), [data]);
  const openingBreakdown = useMemo(() => buildOpeningWinRates(data), [data]);

  if (!data.length) {
    return (
      <section
        role="region"
        aria-label="Player analytics charts"
        className="grid gap-6 xl:grid-cols-2"
      >
        <div className="rounded-2xl border border-dashed border-gray-700 bg-gray-900/40 p-6 text-center text-gray-300 xl:col-span-2">
          <div className="mx-auto mb-3 flex h-12 w-12 items-center justify-center rounded-full bg-teal-500/10 text-teal-300">
            <Sparkles className="h-5 w-5" />
          </div>
          <h3 className="text-lg font-semibold text-white">No analytics yet</h3>
          <p className="mt-2 text-sm text-gray-400">
            Play at least 5 games to unlock rating progression, color split, and repertoire insights.
          </p>
        </div>
      </section>
    );
  }

  return (
    <section
      role="region"
      aria-label="Player analytics charts"
      className="grid gap-6 xl:grid-cols-2"
    >
      <div className="rounded-2xl border border-gray-700/30 bg-gray-900/50 p-4 md:p-6">
        <div className="mb-4 flex items-center justify-between gap-3">
          <div className="flex items-center gap-2 text-sm font-semibold text-white">
            <BarChart3 className="h-4 w-4 text-teal-400" />
            Rating history
          </div>
          <span className="rounded-full border border-gray-700/50 bg-gray-800/70 px-2.5 py-1 text-[10px] uppercase tracking-[0.14em] text-gray-400">
            {range}
          </span>
        </div>
        <div className="h-72 w-full">
          <ResponsiveContainer width="100%" height="100%">
            <BarChart data={ratingHistory} margin={{ top: 10, right: 10, left: 0, bottom: 0 }}>
              <CartesianGrid stroke="rgba(148,163,184,0.12)" vertical={false} />
              <XAxis
                dataKey="date"
                tickFormatter={(value) => new Date(value).toLocaleDateString("en-US", { month: "short", day: "numeric" })}
                tick={{ fill: "#94a3b8", fontSize: 11 }}
                axisLine={false}
                tickLine={false}
                minTickGap={18}
              />
              <YAxis tick={{ fill: "#94a3b8", fontSize: 11 }} axisLine={false} tickLine={false} width={40} />
              <Tooltip
                cursor={{ fill: "rgba(148,163,184,0.08)" }}
                content={({ active, payload }) => {
                  if (!active || !payload?.length) return null;
                  const point = payload[0]?.payload as ChartTooltipEntry["payload"] | undefined;
                  if (!point) return null;

                  return (
                    <div className="rounded-xl border border-gray-700 bg-slate-900/95 px-3 py-2 text-sm text-gray-200 shadow-2xl shadow-black/20">
                      <p className="font-semibold text-white">{point.date ? formatDate(point.date) : "Match"}</p>
                      <p className="mt-1 text-xs text-gray-400">vs {point.opponent ?? "Unknown"}</p>
                      <p className="mt-2 text-xs text-gray-300">
                        Result: <span className="font-medium text-white">{point.result ?? "—"}</span>
                      </p>
                      <p className={`mt-1 text-xs font-semibold ${point.change !== undefined && point.change >= 0 ? "text-emerald-400" : "text-red-400"}`}>
                        Rating delta: {point.change !== undefined ? formatRatingDelta(point.change) : "0"}
                      </p>
                    </div>
                  );
                }}
              />
              <Bar dataKey="elo" radius={[8, 8, 0, 0]} fill="#2dd4bf" />
            </BarChart>
          </ResponsiveContainer>
        </div>
      </div>

      <div className="rounded-2xl border border-gray-700/30 bg-gray-900/50 p-4 md:p-6">
        <div className="mb-4 flex items-center gap-2 text-sm font-semibold text-white">
          <PieChartIcon className="h-4 w-4 text-emerald-400" />
          Color split
        </div>
        <div className="h-72 w-full">
          <ResponsiveContainer width="100%" height="100%">
            <PieChart>
              <Pie data={colorBreakdown} dataKey="games" innerRadius={48} outerRadius={74} paddingAngle={3} stroke="none">
                {colorBreakdown.map((entry, index) => (
                  <Cell key={entry.label} fill={RESULT_COLORS[index]} />
                ))}
              </Pie>
              <Tooltip
                formatter={(value) => {
                  const numeric = Number(Array.isArray(value) ? value[0] : value ?? 0);
                  return [`${numeric} games`, "Games"];
                }}
                contentStyle={{ backgroundColor: "#0f172a", border: "1px solid rgba(148,163,184,0.2)", borderRadius: 12 }}
              />
            </PieChart>
          </ResponsiveContainer>
        </div>
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

      <div className="rounded-2xl border border-gray-700/30 bg-gray-900/50 p-4 md:p-6 xl:col-span-2">
        <div className="mb-4 flex items-center gap-2 text-sm font-semibold text-white">
          <Radar className="h-4 w-4 text-violet-400" />
          Opening repertoire win-rates
        </div>
        <div className="grid gap-6 lg:grid-cols-[1.2fr_0.8fr]">
          <div className="h-80 w-full">
            <ResponsiveContainer width="100%" height="100%">
              <RadarChart data={openingBreakdown.length ? openingBreakdown : DEFAULT_OPENINGS.map((opening) => ({ opening, games: 0, wins: 0, winRate: 0 }))}>
                <PolarGrid stroke="rgba(148,163,184,0.2)" />
                <PolarAngleAxis dataKey="opening" tick={{ fill: "#cbd5e1", fontSize: 11 }} />
                <RechartsRadar dataKey="winRate" stroke="#a78bfa" fill="#8b5cf6" fillOpacity={0.4} />
                <Tooltip
                  formatter={(value) => {
                    const numeric = Number(Array.isArray(value) ? value[0] : value ?? 0);
                    return [`${numeric.toFixed(0)}%`, "Win rate"];
                  }}
                  contentStyle={{ backgroundColor: "#0f172a", border: "1px solid rgba(148,163,184,0.2)", borderRadius: 12 }}
                />
              </RadarChart>
            </ResponsiveContainer>
          </div>

          <div className="space-y-3">
            {openingBreakdown.map((entry) => (
              <div key={entry.opening} className="rounded-xl border border-gray-700/30 bg-gray-800/70 p-3">
                <div className="mb-2 flex items-center justify-between gap-3 text-sm text-gray-200">
                  <span className="font-medium text-white">{entry.opening}</span>
                  <span className="font-semibold text-violet-300">{entry.winRate.toFixed(0)}%</span>
                </div>
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
