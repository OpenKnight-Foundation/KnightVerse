/**
 * CheatDetectionEngine — Client-side heuristic analysis for engine-assisted play.
 *
 * Analyses move time consistency, accuracy patterns, and position complexity
 * to produce a per-player suspicion score.  Designed to be CPU-efficient:
 * all heuristics are O(1) per move with amortised O(n) bookkeeping.
 *
 * NOTE: This is a *heuristic* tool, not a definitive verdict.  It is meant
 * to flag games for further review, not to ban players automatically.
 */

import { Chess, Move } from "chess.js";

// ── Types ──────────────────────────────────────────────────────────────────

export interface MoveEntry {
  /** SAN notation e.g. "Nf3" */
  san: string;
  /** Move timestamp (Date.now()) */
  timestamp: number;
  /** FEN *before* this move was played */
  fenBefore: string;
  /** The chess.js verbose move object */
  verbose: Move;
  /** Which color played this move */
  color: "w" | "b";
  /** Move number (1-based full-move) */
  moveNumber: number;
}

export type RiskLevel = "low" | "moderate" | "high" | "critical";

export interface HeuristicResult {
  /** 0–100 suspicion score (higher = more suspicious) */
  score: number;
  riskLevel: RiskLevel;
  /** Human-readable summary */
  summary: string;
  /** Breakdown by heuristic */
  details: HeuristicDetails;
}

export interface HeuristicDetails {
  /** How consistently the player thinks (lower variance = more suspicious) */
  timeConsistency: number;
  /** How often the player matches the "best" move (from chess.js legality check) */
  accuracyScore: number;
  /** How quickly the player finds complex moves (fast in complex = suspicious) */
  complexitySpeed: number;
  /** Whether the player avoids all blunders (never blundering = suspicious) */
  blunderAvoidance: number;
  /** Blunder count (outright losing moves) */
  blunderCount: number;
  /** Total moves analysed for this colour */
  moveCount: number;
  /** Average thinking time in seconds */
  avgThinkTime: number;
  /** Standard deviation of thinking time */
  thinkTimeStdDev: number;
  /** Best-move match rate (0–1) */
  bestMoveRate: number;
}

// ── Configuration ──────────────────────────────────────────────────────────

const MIN_MOVES_FOR_ANALYSIS = 6; // Need at least 6 moves to start scoring
const SUSPICIOUS_TIME_STDDEV_MS = 1500; // Very consistent thinkers (< 1.5s stddev)
const FAST_MOVE_THRESHOLD_MS = 2000; // Moves played in under 2s
const COMPLEX_POSITION_THRESHOLD = 30; // FEN piece count threshold for "complex"

// ── Engine ─────────────────────────────────────────────────────────────────

export class CheatDetectionEngine {
  private moves: MoveEntry[] = [];
  private lastTimestamp: Record<"w" | "b", number> = { w: 0, b: 0 };

  /**
   * Record a new move.  Call this every time either player moves.
   */
  recordMove(entry: Omit<MoveEntry, "timestamp">): void {
    const now = Date.now();
    this.moves.push({ ...entry, timestamp: now });
    this.lastTimestamp[entry.color] = now;
  }

  /**
   * Get all recorded moves.
   */
  getMoves(): MoveEntry[] {
    return [...this.moves];
  }

  /**
   * Get moves for a specific colour.
   */
  getMovesForColor(color: "w" | "b"): MoveEntry[] {
    return this.moves.filter((m) => m.color === color);
  }

  /**
   * Reset all state.
   */
  reset(): void {
    this.moves = [];
    this.lastTimestamp = { w: 0, b: 0 };
  }

  /**
   * Run heuristic analysis for a given colour.
   * Returns a suspicion score and breakdown.
   */
  analyse(color: "w" | "b"): HeuristicResult {
    const playerMoves = this.getMovesForColor(color);

    if (playerMoves.length < MIN_MOVES_FOR_ANALYSIS) {
      return {
        score: 0,
        riskLevel: "low",
        summary: `Insufficient data (${playerMoves.length}/${MIN_MOVES_FOR_ANALYSIS} moves)`,
        details: this.emptyDetails(playerMoves.length),
      };
    }

    const thinkTimes = this.computeThinkTimes(color);
    const timeConsistency = this.scoreTimeConsistency(thinkTimes);
    const accuracyScore = this.scoreAccuracy(playerMoves);
    const complexitySpeed = this.scoreComplexitySpeed(playerMoves);
    const blunderAvoidance = this.scoreBlunderAvoidance(playerMoves);
    const blunderCount = this.countBlunders(playerMoves);

    const avgThinkTime =
      thinkTimes.length > 0
        ? thinkTimes.reduce((a, b) => a + b, 0) / thinkTimes.length / 1000
        : 0;

    const thinkTimeStdDev = this.stdDev(thinkTimes) / 1000;

    const bestMoveRate = accuracyScore / 100;

    // Weighted composite score
    const composite =
      timeConsistency * 0.25 +
      accuracyScore * 0.3 +
      complexitySpeed * 0.25 +
      blunderAvoidance * 0.2;

    const score = Math.min(100, Math.round(composite));
    const riskLevel = this.classifyRisk(score);

    return {
      score,
      riskLevel,
      summary: this.generateSummary(score, riskLevel, {
        timeConsistency,
        accuracyScore,
        complexitySpeed,
        blunderAvoidance,
        blunderCount,
        moveCount: playerMoves.length,
        avgThinkTime,
        thinkTimeStdDev,
        bestMoveRate,
      }),
      details: {
        timeConsistency,
        accuracyScore,
        complexitySpeed,
        blunderAvoidance,
        blunderCount,
        moveCount: playerMoves.length,
        avgThinkTime,
        thinkTimeStdDev,
        bestMoveRate,
      },
    };
  }

  // ── Private helpers ────────────────────────────────────────────────────

  private emptyDetails(moveCount: number): HeuristicDetails {
    return {
      timeConsistency: 0,
      accuracyScore: 0,
      complexitySpeed: 0,
      blunderAvoidance: 0,
      blunderCount: 0,
      moveCount,
      avgThinkTime: 0,
      thinkTimeStdDev: 0,
      bestMoveRate: 0,
    };
  }

  /**
   * Compute thinking times between consecutive moves of the same colour.
   * Returns times in milliseconds.
   */
  private computeThinkTimes(color: "w" | "b"): number[] {
    const playerMoves = this.getMovesForColor(color);
    const times: number[] = [];

    for (let i = 1; i < playerMoves.length; i++) {
      const prev = playerMoves[i - 1];
      const curr = playerMoves[i];
      // Time between this player's consecutive moves
      // This includes the opponent's thinking time, so we estimate:
      // thinkTime = time since last *own* move minus average opponent think time
      const elapsed = curr.timestamp - prev.timestamp;

      // Heuristic: we don't know opponent think time precisely from client,
      // but we can approximate by halving (assuming roughly equal think times).
      // A more accurate approach would track opponent timestamps separately.
      // For now, we use the raw elapsed time as a proxy — the engine primarily
      // looks at consistency patterns, not absolute values.
      if (elapsed > 0 && elapsed < 300000) {
        // Cap at 5 min to handle disconnections
        times.push(elapsed);
      }
    }

    return times;
  }

  /**
   * Score: How consistently does the player think?
   * Low variance = suspicious (engine always responds at same speed).
   * Returns 0–100 (higher = more suspicious).
   */
  private scoreTimeConsistency(thinkTimes: number[]): number {
    if (thinkTimes.length < 3) return 0;

    const stddev = this.stdDev(thinkTimes);

    // Very consistent (low stddev) → suspicious
    if (stddev < SUSPICIOUS_TIME_STDDEV_MS) {
      // Map 0 stddev → 90, SUSPICIOUS_TIME_STDDEV_MS → 20
      return Math.round(
        90 - (stddev / SUSPICIOUS_TIME_STDDEV_MS) * 70,
      );
    }

    // Normal variation → low suspicion
    return Math.max(0, Math.round(20 - (stddev - SUSPICIOUS_TIME_STDDEV_MS) / 100));
  }

  /**
   * Score: How "accurate" are the player's moves?
   * We use chess.js to check if the move is among the top moves
   * in the position (captures, checks, strong developing moves).
   * Returns 0–100 (higher = more suspicious).
   */
  private scoreAccuracy(playerMoves: MoveEntry[]): number {
    if (playerMoves.length < MIN_MOVES_FOR_ANALYSIS) return 0;

    let bestMoveCount = 0;

    for (const move of playerMoves) {
      if (this.isHighQualityMove(move)) {
        bestMoveCount++;
      }
    }

    const rate = bestMoveCount / playerMoves.length;

    // High accuracy rate → suspicious
    // 80%+ best move rate is extremely suspicious for humans
    // 50-60% is typical for strong players
    if (rate >= 0.85) return Math.min(95, Math.round(rate * 100));
    if (rate >= 0.7) return Math.round(50 + (rate - 0.7) * 200);
    if (rate >= 0.5) return Math.round(20 + (rate - 0.5) * 150);
    return Math.round(rate * 40);
  }

  /**
   * Determine if a move is "high quality" using chess.js heuristics.
   * Checks: is capture, is check, is castle, develops piece, controls center.
   */
  private isHighQualityMove(entry: MoveEntry): boolean {
    const move = entry.verbose;

    // Captures are generally strong
    if (move.captured) return true;
    // Checks are generally strong
    if (move.san.includes("+")) return true;
    // Castling is almost always good
    if (move.san === "O-O" || move.san === "O-O-O") return true;

    // Center pawn moves in opening
    const centerSquares = ["e4", "d4", "e5", "d5"];
    if (
      move.piece === "p" &&
      centerSquares.includes(move.to) &&
      entry.moveNumber <= 10
    ) {
      return true;
    }

    // Knight/bishop development in opening
    if (
      (move.piece === "n" || move.piece === "b") &&
      entry.moveNumber <= 10 &&
      move.from !== move.to
    ) {
      return true;
    }

    // Promotions are always strong
    if (move.promotion) return true;

    return false;
  }

  /**
   * Score: How quickly does the player find moves in complex positions?
   * Fast moves in complex positions = suspicious (engine evaluates instantly).
   * Returns 0–100 (higher = more suspicious).
   */
  private scoreComplexitySpeed(playerMoves: MoveEntry[]): number {
    if (playerMoves.length < MIN_MOVES_FOR_ANALYSIS) return 0;

    // Think time for each complex-position move, measured against this player's *own*
    // previous move. We deliberately compute the gap here rather than indexing into the
    // compacted `thinkTimes` array: that array starts at move 1 and is filtered, so its
    // indices are not aligned with `playerMoves` and would attribute a neighbour's think
    // time to a complex move. The first move (i === 0) has no preceding own move, so its
    // think time is unknown and it is excluded.
    const complexThinkTimes: number[] = [];
    for (let i = 1; i < playerMoves.length; i++) {
      const pieceCount = this.countPieces(playerMoves[i].fenBefore);
      if (pieceCount < COMPLEX_POSITION_THRESHOLD) {
        continue;
      }

      const elapsed = playerMoves[i].timestamp - playerMoves[i - 1].timestamp;
      if (elapsed > 0) {
        complexThinkTimes.push(elapsed);
      }
    }

    if (complexThinkTimes.length < 3) return 0;

    const fastComplexMoves = complexThinkTimes.filter(
      (elapsed) => elapsed < FAST_MOVE_THRESHOLD_MS,
    ).length;
    const fastRate = fastComplexMoves / complexThinkTimes.length;

    // High rate of fast moves in complex positions → suspicious
    if (fastRate >= 0.5) return Math.min(90, Math.round(fastRate * 120));
    if (fastRate >= 0.3) return Math.round(30 + (fastRate - 0.3) * 200);
    return Math.round(fastRate * 100);
  }

  /**
   * Score: How well does the player avoid blunders?
   * Never blundering over many moves is suspicious.
   * Returns 0–100 (higher = more suspicious).
   */
  private scoreBlunderAvoidance(playerMoves: MoveEntry[]): number {
    if (playerMoves.length < MIN_MOVES_FOR_ANALYSIS) return 0;

    const blunderCount = this.countBlunders(playerMoves);
    const moveCount = playerMoves.length;

    // Check for obvious blunders: hanging pieces, bad captures
    // A human player almost always makes at least some inaccuracies
    if (blunderCount === 0 && moveCount >= 15) return 60;
    if (blunderCount === 0 && moveCount >= 10) return 40;

    const blunderRate = blunderCount / moveCount;

    // Very low blunder rate is suspicious
    if (blunderRate < 0.05 && moveCount >= 10) return 50;
    if (blunderRate < 0.1) return 25;

    return Math.max(0, Math.round(15 - blunderRate * 100));
  }

  /**
   * Count obvious blunders: moves that leave pieces hanging.
   */
  private countBlunders(playerMoves: MoveEntry[]): number {
    let blunders = 0;

    for (const move of playerMoves) {
      const chess = new Chess(move.fenBefore);
      const piece = move.verbose.piece;

      // Moving a piece to a square where it can be captured for free
      // is a heuristic blunder indicator
      if (piece !== "p" && piece !== "k") {
        try {
          chess.move(move.san);
          // After the move, check if the moved piece is now en prise
          // (simplified: opponent can capture it)
          const opponentMoves = chess.moves({ verbose: true });
          const targetCapture = opponentMoves.find(
            (m) => m.to === move.verbose.to && m.captured,
          );
          if (targetCapture && !move.verbose.captured) {
            // Piece moved to attacked square without capturing
            // This is a heuristic blunder indicator
            // (Not always accurate - could be a sacrifice or trade)
            // We only count it if the capturing piece is worth less
            const pieceValues: Record<string, number> = {
              p: 1, n: 3, b: 3, r: 5, q: 9, k: 0,
            };
            const movedValue = pieceValues[piece] ?? 0;
            const capturerValue =
              pieceValues[targetCapture.piece] ?? 0;

            if (movedValue > capturerValue + 1) {
              blunders++;
            }
          }
        } catch {
          // Move validation failed - skip
        }
      }
    }

    return blunders;
  }

  /**
   * Count pieces on the board from a FEN string.
   */
  private countPieces(fen: string): number {
    const boardPart = fen.split(" ")[0];
    let count = 0;
    for (const ch of boardPart) {
      if (/[pnbrqkPNBRQK]/.test(ch)) count++;
    }
    return count;
  }

  /**
   * Classify suspicion score into a risk level.
   */
  private classifyRisk(score: number): RiskLevel {
    if (score >= 70) return "critical";
    if (score >= 50) return "high";
    if (score >= 30) return "moderate";
    return "low";
  }

  /**
   * Generate a human-readable summary.
   */
  private generateSummary(
    score: number,
    riskLevel: RiskLevel,
    details: HeuristicDetails,
  ): string {
    const flags: string[] = [];

    if (details.timeConsistency >= 50) {
      flags.push("unusually consistent think times");
    }
    if (details.accuracyScore >= 50) {
      flags.push("high best-move match rate");
    }
    if (details.complexitySpeed >= 40) {
      flags.push("fast moves in complex positions");
    }
    if (details.blunderAvoidance >= 40) {
      flags.push("no detectable blunders");
    }

    if (flags.length === 0) {
      return `Score ${score}/100 — No significant anomalies detected over ${details.moveCount} moves.`;
    }

    return `Score ${score}/100 (${riskLevel}) — Flags: ${flags.join("; ")}. ${details.moveCount} moves analysed.`;
  }

  /**
   * Compute standard deviation of an array of numbers.
   */
  private stdDev(values: number[]): number {
    if (values.length < 2) return 0;
    const mean = values.reduce((a, b) => a + b, 0) / values.length;
    const variance =
      values.reduce((sum, val) => sum + (val - mean) ** 2, 0) /
      (values.length - 1);
    return Math.sqrt(variance);
  }
}

// ── Singleton ──────────────────────────────────────────────────────────────

let engineInstance: CheatDetectionEngine | null = null;

/**
 * Get a shared cheat detection engine instance.
 * Use this for convenience, or create your own with `new CheatDetectionEngine()`.
 */
export function getCheatDetectionEngine(): CheatDetectionEngine {
  if (!engineInstance) {
    engineInstance = new CheatDetectionEngine();
  }
  return engineInstance;
}
