import { describe, it, expect } from "vitest";
import { toPlayerProfile, toRecentGame, playerHref } from "@/lib/playerProfileService";
import {
  buildAnalysis,
  classifyMove,
  evalToCp,
  moveAccuracy,
  toPositionEval,
} from "@/lib/gameAnalysis";

describe("player profile", () => {
  it("normalises ratings and titles", () => {
    expect(
      toPlayerProfile({ username: "bob", ratings: { blitz: 1650.2, rapid: { rating: 1700 } }, title: "GM" }, "x"),
    ).toMatchObject({ username: "bob", ratings: { blitz: 1650, rapid: 1700 }, titles: ["GM"] });
    expect(toPlayerProfile({ elo: 1500 }, "fallback")).toMatchObject({
      username: "fallback",
      ratings: { overall: 1500 },
      titles: [],
    });
  });

  it("normalises recent games and drops incomplete ones", () => {
    expect(toRecentGame({ id: 3, opponent_username: "al", outcome: "won", rating_change: 8 })).toMatchObject({
      id: "3",
      opponent: "al",
      result: "win",
      ratingChange: 8,
    });
    expect(toRecentGame({ id: "1", opponent: "al" })).toBeNull();
  });

  it("encodes usernames in links", () => {
    expect(playerHref("a b")).toBe("/player/a%20b");
  });
});

describe("game analysis (BE-179)", () => {
  it("clamps mate scores", () => {
    expect(evalToCp({ cp: null, mate: 3 })).toBe(1000);
    expect(evalToCp({ cp: -5000, mate: null })).toBe(-1000);
  });

  it("classifies by centipawn loss", () => {
    expect(classifyMove(5, false)).toBe("best");
    expect(classifyMove(400, true)).toBe("best");
    expect(classifyMove(80, false)).toBe("inaccuracy");
    expect(classifyMove(300, false)).toBe("blunder");
  });

  it("gives ~100% accuracy for no loss", () => {
    expect(moveAccuracy(50, 50)).toBeCloseTo(100, 0);
    expect(moveAccuracy(80, 20)).toBeLessThan(10);
  });

  it("builds plies, counts and accuracy from the mover's perspective", () => {
    // 1. e4 (keeps +30) e5 (Black keeps balance) 2. Qh5?? drops to -300
    const a = buildAnalysis(
      ["e4", "e5", "Qh5"],
      ["e2e4", "e7e5", "d1h5"],
      [
        { cp: 30, mate: null, bestMove: "e2e4" },
        { cp: 30, mate: null },
        { cp: 30, mate: null },
        { cp: -300, mate: null },
      ],
      "server",
    );
    expect(a.plies).toHaveLength(4);
    expect(a.plies[1]).toMatchObject({ plyIndex: 1, san: "e4", quality: "best" });
    expect(a.plies[3].quality).toBe("blunder");
    expect(a.counts.blunder).toBe(1);
    expect(a.counts.best).toBe(2);
    expect(a.blackAccuracy).toBeGreaterThan(a.whiteAccuracy);
  });

  it("parses backend evaluation entries", () => {
    expect(toPositionEval({ cp: 12, best_move: "e2e4" })).toEqual({ cp: 12, mate: null, bestMove: "e2e4" });
    expect(toPositionEval(25)).toEqual({ cp: 25, mate: null });
    expect(toPositionEval({})).toBeNull();
  });
});
