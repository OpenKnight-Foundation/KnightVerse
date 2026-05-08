import type { EloDataPoint } from "@/components/profile/EloRatingChart";
import type { EloStats, TimeRange } from "@/hook/useEloStats";

export function filterByTimeRange(data: EloDataPoint[], range: TimeRange): EloDataPoint[] {
  if (!data.length || range === "all") {
    return data;
  }

  const now = new Date(data[data.length - 1].date);
  const cutoff = new Date(now);

  switch (range) {
    case "7d":
      cutoff.setDate(cutoff.getDate() - 7);
      break;
    case "30d":
      cutoff.setDate(cutoff.getDate() - 30);
      break;
    case "90d":
      cutoff.setDate(cutoff.getDate() - 90);
      break;
  }

  return data.filter((point) => new Date(point.date) >= cutoff);
}

export function computeStreak(data: EloDataPoint[]): { current: number; best: number } {
  if (!data.length) {
    return { current: 0, best: 0 };
  }

  let best = 0;
  let currentWinRun = 0;

  for (const point of data) {
    if (point.change > 0) {
      currentWinRun += 1;
      best = Math.max(best, currentWinRun);
    } else {
      currentWinRun = 0;
    }
  }

  const lastChange = data[data.length - 1]?.change ?? 0;

  if (lastChange === 0) {
    return { current: 0, best };
  }

  const direction = lastChange > 0 ? 1 : -1;
  let current = 0;

  for (let index = data.length - 1; index >= 0; index -= 1) {
    const sign = Math.sign(data[index].change);

    if (sign !== direction) {
      break;
    }

    current += 1;
  }

  return {
    current: direction > 0 ? current : -current,
    best,
  };
}

export function computeVolatility(data: EloDataPoint[]): number {
  if (!data.length) {
    return 0;
  }

  const avgChange = data.reduce((sum, point) => sum + point.change, 0) / data.length;
  const variance = data.reduce((sum, point) => sum + (point.change - avgChange) ** 2, 0) / data.length;

  return Math.sqrt(variance);
}

export function computeEloStats(data: EloDataPoint[]): Omit<EloStats, "filteredData"> {
  if (!data.length) {
    return {
      currentElo: 0,
      peakElo: 0,
      lowestElo: 0,
      totalGames: 0,
      wins: 0,
      losses: 0,
      draws: 0,
      winRate: 0,
      currentStreak: 0,
      bestStreak: 0,
      avgChange: 0,
      volatility: 0,
    };
  }

  const wins = data.filter((point) => point.change > 0).length;
  const losses = data.filter((point) => point.change < 0).length;
  const draws = data.length - wins - losses;
  const totalGames = data.length;
  const currentElo = data[data.length - 1]?.elo ?? 0;
  const elos = data.map((point) => point.elo);
  const { current, best } = computeStreak(data);
  const avgChange = data.reduce((sum, point) => sum + point.change, 0) / totalGames;

  return {
    currentElo,
    peakElo: Math.max(...elos),
    lowestElo: Math.min(...elos),
    totalGames,
    wins,
    losses,
    draws,
    winRate: totalGames ? (wins / totalGames) * 100 : 0,
    currentStreak: current,
    bestStreak: best,
    avgChange,
    volatility: computeVolatility(data),
  };
}

export function getRankTier(elo: number): { name: string; color: string; minElo: number; maxElo: number } {
  if (elo < 1000) {
    return { name: "Beginner", color: "slate", minElo: 0, maxElo: 999 };
  }

  if (elo < 1200) {
    return { name: "Intermediate", color: "teal", minElo: 1000, maxElo: 1199 };
  }

  if (elo < 1400) {
    return { name: "Advanced", color: "indigo", minElo: 1200, maxElo: 1399 };
  }

  if (elo < 1600) {
    return { name: "Expert", color: "purple", minElo: 1400, maxElo: 1599 };
  }

  if (elo < 1800) {
    return { name: "Master", color: "amber", minElo: 1600, maxElo: 1799 };
  }

  return { name: "Grandmaster", color: "rose", minElo: 1800, maxElo: Number.POSITIVE_INFINITY };
}
