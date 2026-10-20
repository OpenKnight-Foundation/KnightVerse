import { describe, expect, it } from "vitest";
import {
  dueCards,
  initialCardState,
  qualityFromDrillResult,
  sm2Review,
} from "@/lib/spacedRepetition";

describe("spacedRepetition (SM-2)", () => {
  it("starts new cards due immediately", () => {
    const s = initialCardState(1000);
    expect(s.easiness).toBe(2.5);
    expect(s.nextReview).toBe(1000);
  });

  it("schedules intervals 1 -> 6 -> growing", () => {
    const now = 1_000_000;
    const s1 = sm2Review(initialCardState(now), 5, now);
    expect(s1.interval).toBe(1);
    expect(s1.repetitions).toBe(1);

    const s2 = sm2Review(s1, 5, now);
    expect(s2.interval).toBe(6);
    expect(s2.repetitions).toBe(2);

    const s3 = sm2Review(s2, 5, now);
    expect(s3.interval).toBeGreaterThan(6);
    expect(s3.easiness).toBeLessThanOrEqual(2.5);
  });

  it("resets streak on failure and clamps easiness", () => {
    const now = 2_000_000;
    const good = sm2Review(sm2Review(initialCardState(now), 5, now), 5, now);
    const failed = sm2Review(good, 0, now);
    expect(failed.repetitions).toBe(0);
    expect(failed.interval).toBe(1);
    expect(failed.lapses).toBe(1);
    expect(failed.easiness).toBeGreaterThanOrEqual(1.3);
  });

  it("returns only due cards oldest-first", () => {
    const now = 5_000_000;
    const a = { srs: { ...initialCardState(now - 2000), nextReview: now - 2000 } };
    const b = { srs: { ...initialCardState(now + 5000), nextReview: now + 5000 } };
    const c = { srs: { ...initialCardState(now - 1000), nextReview: now - 1000 } };
    expect(dueCards([b, c, a], now).map((x) => x.srs.nextReview)).toEqual([
      now - 2000,
      now - 1000,
    ]);
  });

  it("maps drill results to quality grades", () => {
    expect(
      qualityFromDrillResult({ correct: true, hintsUsed: 0, attempts: 1 }),
    ).toBe(5);
    expect(
      qualityFromDrillResult({ correct: true, hintsUsed: 1, attempts: 1 }),
    ).toBe(3);
    expect(
      qualityFromDrillResult({ correct: false, hintsUsed: 0, attempts: 1 }),
    ).toBe(2);
  });
});
