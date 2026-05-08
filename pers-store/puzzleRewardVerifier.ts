// On-chain Puzzle Reward Verification (#530)
// Verifies puzzle solutions and authorizes XLM reward payouts via Soroban

import crypto from "crypto";

interface PuzzleSolution {
  puzzleId: string;
  playerId: string;
  moves: string[];
  solvedAt: number;
}

interface RewardClaim {
  claimId: string;
  puzzleId: string;
  playerId: string;
  rewardXlm: number;
  verified: boolean;
  signature: string;
}

const PUZZLE_SOLUTIONS: Record<string, string[]> = {
  "puzzle-001": ["e4", "e5", "Qh5", "Nc6", "Bc4", "Nf6", "Qxf7"],
  "puzzle-002": ["Rxh7", "Kxh7", "Qh5", "Kg8", "Qxf7"],
};

function hashSolution(moves: string[]): string {
  return crypto.createHash("sha256").update(moves.join(",")).digest("hex");
}

function signClaim(claimId: string, playerId: string, rewardXlm: number): string {
  const payload = `${claimId}:${playerId}:${rewardXlm}`;
  return crypto.createHmac("sha256", "xlmate-secret").update(payload).digest("hex");
}

export function verifyPuzzleSolution(solution: PuzzleSolution): boolean {
  const expected = PUZZLE_SOLUTIONS[solution.puzzleId];
  if (!expected) return false;
  return hashSolution(solution.moves) === hashSolution(expected);
}

export function buildRewardClaim(solution: PuzzleSolution, rewardXlm: number): RewardClaim {
  const verified = verifyPuzzleSolution(solution);
  const claimId = crypto.randomUUID();
  return {
    claimId,
    puzzleId: solution.puzzleId,
    playerId: solution.playerId,
    rewardXlm: verified ? rewardXlm : 0,
    verified,
    signature: verified ? signClaim(claimId, solution.playerId, rewardXlm) : "",
  };
}

export function processClaims(solutions: PuzzleSolution[], rewardXlm: number): RewardClaim[] {
  return solutions.map((s) => buildRewardClaim(s, rewardXlm));
}
