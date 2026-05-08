import { renderHook } from "@testing-library/react";
import type { EloDataPoint } from "@/components/profile/EloRatingChart";
import { EXTENDED_MOCK_ELO_DATA } from "@/constants/mockEloData";
import { useEloStats } from "@/hook/useEloStats";

const TEST_DATA: EloDataPoint[] = [
  { date: "2026-03-01", elo: 1108, opponent: "GA", change: 8 },
  { date: "2026-03-04", elo: 1094, opponent: "GB", change: -14 },
  { date: "2026-03-09", elo: 1094, opponent: "GC", change: 0 },
  { date: "2026-03-20", elo: 1111, opponent: "GD", change: 17 },
  { date: "2026-03-26", elo: 1127, opponent: "GE", change: 16 },
];

describe("useEloStats", () => {
  it("returns computed stats for the selected range", () => {
    const { result } = renderHook(() => useEloStats(TEST_DATA, "30d"));

    expect(result.current.currentElo).toBe(1127);
    expect(result.current.totalGames).toBe(5);
    expect(result.current.wins).toBe(3);
    expect(result.current.losses).toBe(1);
    expect(result.current.draws).toBe(1);
    expect(result.current.winRate).toBeCloseTo(60);
  });

  it("memoizes the result when inputs do not change", () => {
    const { result, rerender } = renderHook(
      ({ data, range }) => useEloStats(data, range),
      {
        initialProps: { data: TEST_DATA, range: "30d" as const },
      },
    );
    const firstResult = result.current;

    rerender({ data: TEST_DATA, range: "30d" as const });

    expect(result.current).toBe(firstResult);
  });

  it("filters by time range", () => {
    const { result } = renderHook(() => useEloStats(EXTENDED_MOCK_ELO_DATA, "7d"));

    expect(result.current.filteredData).toHaveLength(8);
    expect(result.current.filteredData[0]?.date).toBe("2026-03-19");
    expect(result.current.filteredData.at(-1)?.date).toBe("2026-03-26");
  });
});
