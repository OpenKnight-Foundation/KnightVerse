/**
 * AI-45: Anti-Cheat Engine Move Correlation Detection across Depth Profiles
 *
 * Evaluates a game's moves against multi-depth engine recommendations
 * (depths 10, 14, 18, 22) to compute a Move Correlation Index (MCI)
 * and Weighted Error Disparity (WED).  High MCI + low WED indicates
 * suspicious engine-assisted play.
 */

export interface DepthEval {
  depth: number;
  /** Top-3 moves the engine recommends at this depth (SAN or UCI) */
  topMoves: [string, string, string];
  /** Centipawn evaluation at this depth */
  score: number;
}

export interface MoveDepthProfile {
  /** The move actually played (SAN/UCI) */
  played: string;
  /** Engine evaluations at each probe depth */
  evals: DepthEval[];
}

export interface AntiCheatReport {
  /** Move Correlation Index 0–1 (higher = more matches across depths) */
  mci: number;
  /** Weighted Error Disparity 0–1 (lower = consistently matches engine) */
  wed: number;
  /** Overall suspicion score 0–100 */
  suspicionScore: number;
  /** 95% confidence interval for the suspicion score */
  confidenceInterval: [number, number];
  verdict: "clean" | "suspicious" | "likely_cheating";
  summary: string;
}

const PROBE_DEPTHS = [10, 14, 18, 22] as const;

/**
 * Compute MCI: fraction of moves where the played move appears in the
 * engine's top-3 at ALL probe depths simultaneously.
 */
function computeMCI(profiles: MoveDepthProfile[]): number {
  if (profiles.length === 0) return 0;
  const matched = profiles.filter((p) =>
    p.evals.every((e) => e.topMoves.includes(p.played)),
  ).length;
  return matched / profiles.length;
}

/**
 * Compute WED: average centipawn loss weighted by depth — deeper evals
 * are more accurate so their discrepancy matters more.
 */
function computeWED(profiles: MoveDepthProfile[]): number {
  if (profiles.length === 0) return 0;
  const depthWeights: Record<number, number> = { 10: 0.1, 14: 0.2, 18: 0.3, 22: 0.4 };
  let totalWeighted = 0;
  let totalWeight = 0;
  for (const p of profiles) {
    for (const e of p.evals) {
      const w = depthWeights[e.depth] ?? 0.1;
      const bestScore = e.score;
      // If played move is top-1, no loss; otherwise approximate 0.1 cp loss per rank drop
      const rank = e.topMoves.indexOf(p.played);
      const loss = rank < 0 ? 50 : rank * 10;
      totalWeighted += loss * w;
      totalWeight += w * 100; // normalise against 100 cp
    }
  }
  return totalWeight > 0 ? Math.min(1, totalWeighted / totalWeight) : 0;
}

/**
 * Generate a fair-play report for a game given per-move depth profiles.
 *
 * @param profiles  One entry per move containing the played move and
 *                  engine evaluations at each of PROBE_DEPTHS.
 */
export function generateFairPlayReport(profiles: MoveDepthProfile[]): AntiCheatReport {
  const mci = computeMCI(profiles);
  const wed = computeWED(profiles);

  // Suspicion increases with MCI and decreases with WED
  const raw = mci * 70 + (1 - wed) * 30;
  const suspicionScore = Math.round(Math.min(100, Math.max(0, raw)));

  // Simple ±5 point 95% CI based on sample size
  const margin = profiles.length > 20 ? 3 : 7;
  const confidenceInterval: [number, number] = [
    Math.max(0, suspicionScore - margin),
    Math.min(100, suspicionScore + margin),
  ];

  const verdict =
    suspicionScore >= 80
      ? "likely_cheating"
      : suspicionScore >= 55
        ? "suspicious"
        : "clean";

  const summary =
    verdict === "likely_cheating"
      ? `High engine correlation detected (MCI=${mci.toFixed(2)}, WED=${wed.toFixed(2)}). Flagged for review.`
      : verdict === "suspicious"
        ? `Elevated correlation across depth profiles (MCI=${mci.toFixed(2)}). Further review recommended.`
        : `No significant engine correlation detected (MCI=${mci.toFixed(2)}).`;

  return { mci, wed, suspicionScore, confidenceInterval, verdict, summary };
}

export { PROBE_DEPTHS };
