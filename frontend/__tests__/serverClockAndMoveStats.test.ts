import { describe, it, expect } from "vitest";
import { parseClock, projectClock } from "@/hook/useChessSocket";
import { toMoveStat } from "@/lib/openingMoveStats";

describe("server clock (BE-149)", () => {
  it("parses ms clock fields from any frame", () => {
    const c = parseClock({ type: "move", whiteMs: 60000, blackMs: 59000, turn: "b" }, 100);
    expect(c).toMatchObject({ whiteMs: 60000, blackMs: 59000, turn: "b", receivedAt: 100 });
  });

  it("accepts seconds only on explicit clock frames", () => {
    expect(parseClock({ type: "move", whiteTime: 60, blackTime: 60 }, 0)).toBeNull();
    expect(parseClock({ type: "clock", whiteTime: 60, blackTime: 50, turn: "white" }, 0))
      .toMatchObject({ whiteMs: 60000, blackMs: 50000, turn: "w" });
  });

  it("projects only the running side, correcting for half the RTT", () => {
    const clock = { whiteMs: 10000, blackMs: 10000, turn: "w" as const, receivedAt: 1000 };
    // 500ms since receipt + 200ms RTT / 2 = 600ms elapsed
    const r = projectClock(clock, 1500, 0, 200, 0);
    expect(r).toEqual({ whiteMs: 9400, blackMs: 10000 });
  });

  it("uses the server timestamp when a clock offset is known", () => {
    const clock = {
      whiteMs: 10000, blackMs: 10000, turn: "b" as const, serverTime: 5000, receivedAt: 0,
    };
    // local epoch 4800 + offset 500 = server now 5300 → 300ms elapsed
    expect(projectClock(clock, 999, 4800, 50, 500)).toEqual({ whiteMs: 10000, blackMs: 9700 });
  });

  it("never goes below zero", () => {
    const clock = { whiteMs: 100, blackMs: 100, turn: "w" as const, receivedAt: 0 };
    expect(projectClock(clock, 10000, 0, 0, 0).whiteMs).toBe(0);
  });
});

describe("opening move stats (BE-178)", () => {
  it("normalises lichess-style entries", () => {
    expect(toMoveStat({ san: "e4", uci: "e2e4", white: 5, draws: 3, black: 2 })).toEqual({
      san: "e4", uci: "e2e4", white: 5, draws: 3, black: 2, games: 10,
    });
  });

  it("drops entries without games or SAN", () => {
    expect(toMoveStat({ san: "e4", white: 0, draws: 0, black: 0 })).toBeNull();
    expect(toMoveStat({ white: 1 })).toBeNull();
  });
});
