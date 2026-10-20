import { describe, expect, it } from "vitest";
import {
  classifyOpening,
  ECO_BOOK,
  statsForEco,
  winRateBar,
} from "@/lib/openings";
import { exportLinesToPgn, importPgnToLines, createLine } from "@/lib/repertoireStore";

describe("openings book", () => {
  it("has ECO entries with moves", () => {
    expect(ECO_BOOK.length).toBeGreaterThan(10);
    for (const o of ECO_BOOK) {
      expect(o.eco).toMatch(/^[A-E]\d{2}$/);
      expect(o.moves.length).toBeGreaterThan(0);
    }
  });

  it("classifies Italian game line", () => {
    const found = classifyOpening(["e4", "e5", "Nf3", "Nc6", "Bc4", "Bc5"]);
    expect(found?.eco).toBe("C50");
  });

  it("classifies longest prefix (Najdorf over base Sicilian)", () => {
    const found = classifyOpening([
      "e4", "c5", "Nf3", "d6", "d4", "cxd4", "Nxd4", "Nf6", "Nc3", "a6",
    ]);
    expect(found?.eco).toBe("B90");
  });

  it("returns null for empty / unknown", () => {
    expect(classifyOpening([])).toBeNull();
  });

  it("provides stats + 100% bar", () => {
    const s = statsForEco("C50");
    expect(s.games).toBeGreaterThan(0);
    const bar = winRateBar(s);
    expect(bar.whitePct + bar.drawPct + bar.blackPct).toBeGreaterThan(95);
  });
});

describe("repertoire PGN round-trip", () => {
  it("exports then re-imports a line", () => {
    const line = createLine({
      name: "Italian Game: Giuoco Piano",
      eco: "C50",
      moves: ["e4", "e5", "Nf3", "Nc6", "Bc4", "Bc5"],
    });
    const pgn = exportLinesToPgn([line], "white");
    expect(pgn).toContain('[ECO "C50"]');
    const back = importPgnToLines(pgn);
    expect(back.length).toBe(1);
    expect(back[0].slice(0, 6)).toEqual(["e4", "e5", "Nf3", "Nc6", "Bc4", "Bc5"]);
  });

  it("skips malformed PGN gracefully", () => {
    expect(importPgnToLines("not a game at all")).toEqual([]);
  });
});
