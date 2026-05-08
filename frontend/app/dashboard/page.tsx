"use client";

import { useState } from "react";
import dynamic from "next/dynamic";
import {
  BarChart3,
  Flame,
  Target,
  TrendingDown,
  TrendingUp,
  Trophy,
} from "lucide-react";
import RankBadge from "@/components/dashboard/RankBadge";
import RecentMatches from "@/components/dashboard/RecentMatches";
import StatCard from "@/components/dashboard/StatCard";
import { EXTENDED_MOCK_ELO_DATA } from "@/constants/mockEloData";
import { useEloStats, type TimeRange } from "@/hook/useEloStats";
import { cn } from "@/lib/utils";

const EloChart = dynamic(() => import("@/components/dashboard/EloChart"), {
  ssr: false,
});
const PerformanceBreakdown = dynamic(() => import("@/components/dashboard/PerformanceBreakdown"), {
  ssr: false,
});

const TIME_RANGES: Array<{ label: string; value: TimeRange }> = [
  { label: "7D", value: "7d" },
  { label: "30D", value: "30d" },
  { label: "90D", value: "90d" },
  { label: "ALL", value: "all" },
];

export default function DashboardPage() {
  const [range, setRange] = useState<TimeRange>("30d");
  const stats = useEloStats(EXTENDED_MOCK_ELO_DATA, range);
  const filteredData = stats.filteredData;
  const baselineElo = filteredData[0]?.elo ?? stats.currentElo;
  const netChange = stats.currentElo - baselineElo;
  const subtitle = `${stats.currentElo} rating · ${stats.totalGames} games in view`;

  return (
    <div className="space-y-6 pb-8" role="region" aria-label="ELO progress dashboard">
      <section className="animate-slide-up rounded-2xl border border-gray-700/30 bg-gray-800/40 p-6 shadow-2xl shadow-black/10">
        <div className="flex flex-col gap-6 xl:flex-row xl:items-start xl:justify-between">
          <div className="space-y-4">
            <div>
              <div className="inline-flex items-center gap-2 rounded-full border border-teal-500/20 bg-teal-500/10 px-3 py-1 text-xs uppercase tracking-[0.18em] text-teal-300">
                <span className="h-2 w-2 rounded-full bg-teal-400 animate-pulse-glow" />
                Rating intelligence
              </div>
              <h1 className="mt-4 text-3xl font-bold text-white md:text-4xl">ELO Progress</h1>
              <p className="mt-2 text-base text-gray-300">{subtitle}</p>
            </div>

            <div className="flex flex-wrap gap-2" role="tablist" aria-label="Dashboard time range filters">
              {TIME_RANGES.map((option) => (
                <button
                  key={option.value}
                  type="button"
                  role="tab"
                  aria-selected={range === option.value}
                  aria-label={`Show ${option.label} ELO data`}
                  onClick={() => setRange(option.value)}
                  className={cn(
                    "rounded-full border px-4 py-2 text-sm font-semibold transition-all duration-300",
                    range === option.value
                      ? "border-teal-400/40 bg-gradient-to-r from-teal-500 to-blue-700 text-white shadow-lg shadow-teal-900/30"
                      : "border-gray-700/50 bg-gray-900/50 text-gray-300 hover:border-gray-600 hover:bg-gray-800/70 hover:text-white",
                  )}
                >
                  {option.label}
                </button>
              ))}
            </div>
          </div>

          <div className="w-full max-w-sm xl:min-w-[320px]">
            <RankBadge elo={stats.currentElo} />
          </div>
        </div>
      </section>

      <section className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4" aria-label="ELO summary statistics">
        <StatCard
          icon={<BarChart3 className="h-6 w-6" />}
          label="Current ELO"
          value={stats.currentElo}
          trend={{ value: netChange, label: "range" }}
          accentColor="teal"
        />
        <StatCard
          icon={<Trophy className="h-6 w-6" />}
          label="Peak Rating"
          value={stats.peakElo}
          trend={{ value: stats.peakElo - stats.currentElo, label: "to peak" }}
          accentColor="blue"
        />
        <StatCard
          icon={<Target className="h-6 w-6" />}
          label="Win Rate"
          value={`${stats.winRate.toFixed(1)}%`}
          trend={{ value: stats.avgChange, label: "avg pts" }}
          accentColor="emerald"
        />
        <StatCard
          icon={stats.currentStreak >= 0 ? <TrendingUp className="h-6 w-6" /> : <TrendingDown className="h-6 w-6" />}
          label="Current Streak"
          value={`${stats.currentStreak > 0 ? "+" : ""}${stats.currentStreak}`}
          trend={{ value: stats.volatility, label: "volatility" }}
          accentColor="amber"
        />
      </section>

      <EloChart data={filteredData} title="Interactive ELO trajectory" height={390} />

      <section className="grid gap-6 xl:grid-cols-[1.35fr_0.9fr]">
        <RecentMatches data={filteredData} />
        <div className="space-y-6">
          <div className="rounded-xl border border-gray-700/30 bg-gray-800/40 p-6 shadow-lg shadow-black/10">
            <div className="mb-4 flex items-center gap-2 text-sm font-semibold text-white">
              <Flame className="h-4 w-4 text-orange-400" />
              Range insights
            </div>
            <div className="grid gap-4 sm:grid-cols-3 xl:grid-cols-1">
              <div className="rounded-xl border border-gray-700/30 bg-gray-900/50 p-4">
                <p className="text-xs uppercase tracking-[0.18em] text-gray-500">Lowest point</p>
                <p className="mt-2 text-2xl font-bold text-white">{stats.lowestElo}</p>
              </div>
              <div className="rounded-xl border border-gray-700/30 bg-gray-900/50 p-4">
                <p className="text-xs uppercase tracking-[0.18em] text-gray-500">Games sampled</p>
                <p className="mt-2 text-2xl font-bold text-white">{stats.totalGames}</p>
              </div>
              <div className="rounded-xl border border-gray-700/30 bg-gray-900/50 p-4">
                <p className="text-xs uppercase tracking-[0.18em] text-gray-500">Best streak</p>
                <p className="mt-2 text-2xl font-bold text-white">{stats.bestStreak}</p>
              </div>
            </div>
          </div>
        </div>
      </section>

      <PerformanceBreakdown
        data={filteredData}
        wins={stats.wins}
        losses={stats.losses}
        draws={stats.draws}
        currentStreak={stats.currentStreak}
        bestStreak={stats.bestStreak}
      />
    </div>
  );
}
