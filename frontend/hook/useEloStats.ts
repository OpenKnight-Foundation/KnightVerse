"use client";

import { useMemo } from "react";
import type { EloDataPoint } from "@/components/profile/EloRatingChart";
import { computeEloStats, filterByTimeRange } from "@/lib/eloStatsUtils";

export type TimeRange = "7d" | "30d" | "90d" | "all";

export interface EloStats {
  currentElo: number;
  peakElo: number;
  lowestElo: number;
  totalGames: number;
  wins: number;
  losses: number;
  draws: number;
  winRate: number;
  currentStreak: number;
  bestStreak: number;
  avgChange: number;
  volatility: number;
  filteredData: EloDataPoint[];
}

export function useEloStats(data: EloDataPoint[], range: TimeRange): EloStats {
  return useMemo(() => {
    const filteredData = filterByTimeRange(data, range);

    return {
      ...computeEloStats(filteredData),
      filteredData,
    };
  }, [data, range]);
}
