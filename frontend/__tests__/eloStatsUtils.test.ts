import { EXTENDED_MOCK_ELO_DATA } from "@/constants/mockEloData";
import {
  buildColorWinBreakdown,
  buildOpeningWinRates,
  computeEloStats,
  computeStreak,
  computeVolatility,
  filterByTimeRange,
  formatRatingDelta,
  getRankTier,
} from "@/lib/eloStatsUtils";
import type { EloDataPoint } from "@/components/profile/EloRatingChart";

const SAMPLE_DATA: EloDataPoint[] = [
  { date: "2026-03-01", elo: 1215, opponent: "GA", change: 15 },
  { date: "2026-03-02", elo: 1205, opponent: "GB", change: -10 },
  { date: "2026-03-03", elo: 1205, opponent: "GC", change: 0 },
  { date: "2026-03-04", elo: 1223, opponent: "GD", change: 18 },
  { date: "2026-03-05", elo: 1239, opponent: "GE", change: 16 },
];

describe("filterByTimeRange", () => {
  it("returns the correct subset for each range", () => {
    expect(filterByTimeRange(EXTENDED_MOCK_ELO_DATA, "7d")).toHaveLength(8);
    expect(filterByTimeRange(EXTENDED_MOCK_ELO_DATA, "30d")).toHaveLength(31);
    expect(filterByTimeRange(EXTENDED_MOCK_ELO_DATA, "90d")).toHaveLength(90);
    expect(filterByTimeRange(EXTENDED_MOCK_ELO_DATA, "1y")).toHaveLength(90);
    expect(filterByTimeRange(EXTENDED_MOCK_ELO_DATA, "all")).toHaveLength(90);
  });

  it("returns an empty list for empty data", () => {
    expect(filterByTimeRange([], "30d")).toEqual([]);
  });
});

describe("analytics helpers", () => {
  it("formats rating deltas consistently", () => {
    expect(formatRatingDelta(12)).toBe("+12");
    expect(formatRatingDelta(-8)).toBe("-8");
    expect(formatRatingDelta(0)).toBe("0");
  });

  it("builds color-specific win rate summaries", () => {
    const breakdown = buildColorWinBreakdown(SAMPLE_DATA);

    expect(breakdown).toHaveLength(2);
    expect(breakdown[0]).toMatchObject({ label: "White", games: 3, wins: 2, winRate: 66.66666666666666 });
    expect(breakdown[1]).toMatchObject({ label: "Black", games: 2, wins: 1, winRate: 50 });
  });

  it("builds opening win-rate data from sample history", () => {
    const openingData = buildOpeningWinRates(SAMPLE_DATA);

    expect(openingData.length).toBeGreaterThan(0);
    expect(openingData[0]).toHaveProperty("opening");
    expect(openingData[0]).toHaveProperty("winRate");
    expect(openingData[0].winRate).toBeGreaterThanOrEqual(0);
  });
});

describe("computeEloStats", () => {
  it("computes aggregate rating metrics", () => {
    const stats = computeEloStats(SAMPLE_DATA);

    expect(stats.currentElo).toBe(1239);
    expect(stats.peakElo).toBe(1239);
    expect(stats.lowestElo).toBe(1205);
    expect(stats.totalGames).toBe(5);
    expect(stats.wins).toBe(3);
    expect(stats.losses).toBe(1);
    expect(stats.draws).toBe(1);
    expect(stats.winRate).toBeCloseTo(60);
    expect(stats.currentStreak).toBe(2);
    expect(stats.bestStreak).toBe(2);
    expect(stats.avgChange).toBeCloseTo(7.8);
    expect(stats.volatility).toBeCloseTo(computeVolatility(SAMPLE_DATA));
  });

  it("handles empty arrays", () => {
    expect(computeEloStats([])).toEqual({
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
    });
  });

  it("handles a single data point", () => {
    const single = computeEloStats([{ date: "2026-03-01", elo: 1120, opponent: "GA", change: 20 }]);

    expect(single.currentElo).toBe(1120);
    expect(single.totalGames).toBe(1);
    expect(single.wins).toBe(1);
    expect(single.currentStreak).toBe(1);
    expect(single.bestStreak).toBe(1);
  });
});

describe("computeStreak", () => {
  it("returns best and current streaks for all wins", () => {
    const data = SAMPLE_DATA.filter((point) => point.change > 0);
    expect(computeStreak(data)).toEqual({ current: 3, best: 3 });
  });

  it("returns current loss streak for trailing losses", () => {
    const data: EloDataPoint[] = [
      { date: "2026-03-01", elo: 1200, opponent: "GA", change: 12 },
      { date: "2026-03-02", elo: 1188, opponent: "GB", change: -12 },
      { date: "2026-03-03", elo: 1171, opponent: "GC", change: -17 },
    ];

    expect(computeStreak(data)).toEqual({ current: -2, best: 1 });
  });

  it("resets current streak on a draw", () => {
    const data: EloDataPoint[] = [
      { date: "2026-03-01", elo: 1200, opponent: "GA", change: 12 },
      { date: "2026-03-02", elo: 1200, opponent: "GB", change: 0 },
    ];

    expect(computeStreak(data)).toEqual({ current: 0, best: 1 });
  });
});

describe("computeVolatility", () => {
  it("returns zero for uniform changes", () => {
    const data = [1, 2, 3].map((index) => ({
      date: `2026-03-0${index}`,
      elo: 1200 + index * 10,
      opponent: `G${index}`,
      change: 10,
    }));

    expect(computeVolatility(data)).toBe(0);
  });

  it("returns a positive standard deviation for varied changes", () => {
    expect(computeVolatility(SAMPLE_DATA)).toBeCloseTo(10.9618, 3);
  });
});

describe("getRankTier", () => {
  it("handles boundary values", () => {
    expect(getRankTier(999).name).toBe("Beginner");
    expect(getRankTier(1000).name).toBe("Intermediate");
    expect(getRankTier(1200).name).toBe("Advanced");
    expect(getRankTier(1400).name).toBe("Expert");
    expect(getRankTier(1600).name).toBe("Master");
    expect(getRankTier(1800).name).toBe("Grandmaster");
  });
});
