// Proof of Game via SHA-256 Move History Hashing (#527)
// Produces a tamper-evident on-chain proof of a completed chess game

import crypto from "crypto";

interface GameRecord {
  gameId: string;
  whitePlayerId: string;
  blackPlayerId: string;
  moves: string[];       // SAN notation, e.g. ["e4", "e5", "Nf3"]
  result: "1-0" | "0-1" | "1/2-1/2";
  startedAt: number;
  endedAt: number;
}

interface GameProof {
  gameId: string;
  moveHash: string;       // SHA-256 of the full move list
  metaHash: string;       // SHA-256 of game metadata
  proofHash: string;      // SHA-256 of moveHash + metaHash (final proof)
  generatedAt: number;
}

function hashMoves(moves: string[]): string {
  const payload = moves.join(" ");
  return crypto.createHash("sha256").update(payload).digest("hex");
}

function hashMetadata(record: GameRecord): string {
  const meta = [
    record.gameId,
    record.whitePlayerId,
    record.blackPlayerId,
    record.result,
    record.startedAt.toString(),
    record.endedAt.toString(),
  ].join("|");
  return crypto.createHash("sha256").update(meta).digest("hex");
}

export function generateGameProof(record: GameRecord): GameProof {
  const moveHash = hashMoves(record.moves);
  const metaHash = hashMetadata(record);
  const proofHash = crypto
    .createHash("sha256")
    .update(moveHash + metaHash)
    .digest("hex");

  return {
    gameId: record.gameId,
    moveHash,
    metaHash,
    proofHash,
    generatedAt: Date.now(),
  };
}

export function verifyGameProof(record: GameRecord, proof: GameProof): boolean {
  const recomputed = generateGameProof(record);
  return recomputed.proofHash === proof.proofHash;
}
