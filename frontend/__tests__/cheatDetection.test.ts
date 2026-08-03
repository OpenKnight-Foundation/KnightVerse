import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Move } from "chess.js";
import { CheatDetectionEngine } from "@/lib/cheatDetection";

// Board part with 10 piece letters -> below COMPLEX_POSITION_THRESHOLD (30).
const SIMPLE_FEN = "rnbqk3/8/8/8/8/8/8/RNBQK3 w - - 0 1";
// Full starting board -> 32 piece letters -> a "complex" position.
const COMPLEX_FEN = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

// Minimal pawn move: piece "p" keeps countBlunders from constructing a Chess board,
// and a non-central destination keeps isHighQualityMove false. Only fenBefore piece
// count and timestamps matter for the complexity-speed heuristic.
function pawnMove(): Move {
  return { color: "w", piece: "p", from: "a2", to: "a3", san: "a3", flags: "n" } as unknown as Move;
}

describe("CheatDetectionEngine complexity-speed think-time alignment", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("attributes each complex move's own think time, not a neighbour's", () => {
    const engine = new CheatDetectionEngine();

    // Single colour throughout. Every complex position was actually played SLOWLY (5000ms);
    // the one fast gap (500ms) precedes a NON-complex move. Correct alignment must therefore
    // report zero fast complex moves. The previous off-by-one indexing pulled the 500ms gap
    // onto a complex move and produced a non-zero complexity-speed score.
    const moves: Array<{ fen: string; t: number }> = [
      { fen: SIMPLE_FEN, t: 0 },
      { fen: SIMPLE_FEN, t: 1000 },
      { fen: COMPLEX_FEN, t: 6000 }, // think 5000ms (slow)
      { fen: COMPLEX_FEN, t: 11000 }, // think 5000ms (slow)
      { fen: COMPLEX_FEN, t: 16000 }, // think 5000ms (slow)
      { fen: SIMPLE_FEN, t: 16500 }, // think 500ms (fast, but NOT complex)
      { fen: SIMPLE_FEN, t: 21500 },
    ];

    moves.forEach((m, i) => {
      vi.setSystemTime(new Date(m.t));
      engine.recordMove({
        san: "a3",
        fenBefore: m.fen,
        verbose: pawnMove(),
        color: "w",
        moveNumber: 11 + i,
      });
    });

    const result = engine.analyse("w");

    expect(result.details.complexitySpeed).toBe(0);
  });

  it("flags genuinely fast play in complex positions", () => {
    const engine = new CheatDetectionEngine();

    // Here the complex positions themselves were all played quickly (500ms each).
    const moves: Array<{ fen: string; t: number }> = [
      { fen: SIMPLE_FEN, t: 0 },
      { fen: SIMPLE_FEN, t: 5000 },
      { fen: COMPLEX_FEN, t: 5500 }, // think 500ms (fast)
      { fen: COMPLEX_FEN, t: 6000 }, // think 500ms (fast)
      { fen: COMPLEX_FEN, t: 6500 }, // think 500ms (fast)
      { fen: COMPLEX_FEN, t: 7000 }, // think 500ms (fast)
      { fen: SIMPLE_FEN, t: 12000 },
    ];

    moves.forEach((m, i) => {
      vi.setSystemTime(new Date(m.t));
      engine.recordMove({
        san: "a3",
        fenBefore: m.fen,
        verbose: pawnMove(),
        color: "w",
        moveNumber: 11 + i,
      });
    });

    const result = engine.analyse("w");

    expect(result.details.complexitySpeed).toBeGreaterThan(0);
  });
});
