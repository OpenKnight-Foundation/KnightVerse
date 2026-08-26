import { describe, expect, it } from "vitest";
import {
  calculateDrawRatings,
  calculateMatchRatings,
  calculateNewRating,
  expectedScore,
  kFactor,
} from "@/lib/eloService";

describe("kFactor", () => {
  it("returns 40 for new players", () => {
    expect(kFactor(1500, 10)).toBe(40);
  });

  it("returns 40 for rating < 1000", () => {
    expect(kFactor(800, 100)).toBe(40);
  });

  it("returns 20 for established players below 2400", () => {
    expect(kFactor(1500, 50)).toBe(20);
  });

  it("returns 10 for elite players", () => {
    expect(kFactor(2500, 200)).toBe(10);
  });
});

describe("expectedScore", () => {
  it("returns 0.5 for equal ratings", () => {
    expect(expectedScore(1500, 1500)).toBeCloseTo(0.5);
  });

  it("returns > 0.5 when player is stronger", () => {
    expect(expectedScore(1600, 1400)).toBeGreaterThan(0.5);
  });

  it("returns < 0.5 when player is weaker", () => {
    expect(expectedScore(1400, 1600)).toBeLessThan(0.5);
  });
});

describe("calculateNewRating", () => {
  it("equal ratings win gives +16 with K=32", () => {
    const { delta } = calculateNewRating(1500, 1500, "win", 32);
    expect(delta).toBe(16);
  });

  it("equal ratings loss gives -16 with K=32", () => {
    const { delta } = calculateNewRating(1500, 1500, "loss", 32);
    expect(delta).toBe(-16);
  });

  it("equal ratings draw gives 0 with K=32", () => {
    const { delta } = calculateNewRating(1500, 1500, "draw", 32);
    expect(delta).toBe(0);
  });

  it("strong player beating weak player gains very little", () => {
    const { delta } = calculateNewRating(3000, 100, "win", 32);
    expect(delta).toBe(0);
  });

  it("weak player upsetting strong player gains ~K", () => {
    const { delta } = calculateNewRating(100, 3000, "win", 32);
    expect(delta).toBe(32);
  });

  it("rating never drops below 100", () => {
    const { newRating } = calculateNewRating(100, 3000, "loss", 40);
    expect(newRating).toBe(100);
  });
});

describe("calculateMatchRatings", () => {
  it("winner gains and loser loses symmetrically for equal ratings", () => {
    const { winner, loser } = calculateMatchRatings(1500, 1500, 50, 50);
    expect(winner.delta).toBeGreaterThan(0);
    expect(loser.delta).toBeLessThan(0);
    expect(winner.newRating).toBe(1500 + winner.delta);
    expect(loser.newRating).toBe(1500 + loser.delta);
  });
});

describe("calculateDrawRatings", () => {
  it("equal ratings draw produces zero delta", () => {
    const { player1, player2 } = calculateDrawRatings(1500, 1500, 50, 50);
    expect(player1.delta).toBe(0);
    expect(player2.delta).toBe(0);
  });

  it("weaker player gains rating on draw against stronger player", () => {
    const { player1 } = calculateDrawRatings(1200, 1800, 50, 50);
    expect(player1.delta).toBeGreaterThan(0);
  });
});
