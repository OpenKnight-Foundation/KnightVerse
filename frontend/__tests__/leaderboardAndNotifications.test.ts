import { describe, it, expect } from "vitest";
import { tierForRating, toLeaderboardRow } from "@/lib/leaderboardService";
import { formatRelativeTime, toNotification } from "@/context/NotificationsContext";

describe("leaderboard rows (BE-158)", () => {
  it("normalises snake_case entries and derives tier and win rate", () => {
    expect(
      toLeaderboardRow({ username: "magnus", elo: 2501.4, wins: 30, games_played: 40 }, 1),
    ).toEqual({ rank: 1, player: "magnus", tier: "Grandmaster", rating: 2501, winRate: 75 });
  });

  it("keeps a backend tier and rank, accepts fractional win rates", () => {
    expect(
      toLeaderboardRow({ rank: 7, player: "a", rating: 1500, tier: "gold", win_rate: 0.42 }, 1),
    ).toMatchObject({ rank: 7, tier: "Gold", winRate: 42 });
  });

  it("rejects rows without a name or rating", () => {
    expect(toLeaderboardRow({ rating: 1500 }, 1)).toBeNull();
    expect(toLeaderboardRow({ username: "x" }, 1)).toBeNull();
  });

  it("maps ratings to tiers", () => {
    expect(tierForRating(1000)).toBe("Bronze");
    expect(tierForRating(2200)).toBe("Master");
  });
});

describe("notifications (BE-163)", () => {
  it("normalises a backend payload", () => {
    const n = toNotification({
      id: 12,
      type: "tournament_round_started",
      message: "Round 3 begins",
      created_at: "2026-01-01T00:00:00Z",
      read_at: null,
      link: "/tournament/1",
    });
    expect(n).toMatchObject({
      id: "12",
      kind: "tournament_round",
      title: "Tournament round starting",
      body: "Round 3 begins",
      href: "/tournament/1",
      read: false,
      createdAt: Date.parse("2026-01-01T00:00:00Z"),
    });
  });

  it("treats read_at as read and seconds timestamps as epoch seconds", () => {
    const n = toNotification({ id: "a", kind: "payout", read_at: "x", created_at: 1_700_000_000 });
    expect(n).toMatchObject({ kind: "payout", read: true, createdAt: 1_700_000_000_000 });
  });

  it("requires an id", () => {
    expect(toNotification({ title: "x" })).toBeNull();
  });

  it("formats relative time", () => {
    expect(formatRelativeTime(0, 30_000)).toBe("just now");
    expect(formatRelativeTime(0, 5 * 60_000)).toBe("5m ago");
    expect(formatRelativeTime(0, 3 * 3_600_000)).toBe("3h ago");
  });
});
