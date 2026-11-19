/**
 * Per-move evaluations for the game review page (BE-179).
 *
 * 1. Ask the backend for stored analysis: `GET /v1/games/{id}/analysis`.
 * 2. Otherwise analyse locally with the bundled Stockfish worker
 *    (`/assets/stockfish.js`) at a shallow depth.
 *
 * Evaluations are normalised to centipawns from White's perspective, then
 * classified and turned into accuracy figures.
 */

import { Chess } from "chess.js";
import { endpoints } from "./api";
import type { EvaluatedPly, MoveQuality } from "@/components/chess/CentipawnAdvantageChart";
import type { MoveClass } from "@/components/chess/GameReviewSummary";

export interface PositionEval {
  /** Centipawns, White's perspective. */
  cp: number | null;
  /** Mate in N, positive = White mates. */
  mate: number | null;
  bestMove?: string;
}

export interface GameAnalysis {
  plies: EvaluatedPly[];
  whiteAccuracy: number;
  blackAccuracy: number;
  counts: Record<MoveClass, number>;
  source: "server" | "local";
}

const MATE_CP = 1000;

/** Evaluation as a clamped centipawn number (mate → ±1000). */
export function evalToCp(e: PositionEval): number {
  if (e.mate != null) return e.mate > 0 ? MATE_CP : -MATE_CP;
  return Math.max(-MATE_CP, Math.min(MATE_CP, e.cp ?? 0));
}

/** Lichess win-percentage model. */
export function winPercent(cp: number): number {
  return 50 + 50 * (2 / (1 + Math.exp(-0.00368208 * cp)) - 1);
}

/** Lichess per-move accuracy from win-percentage drop. */
export function moveAccuracy(winBefore: number, winAfter: number): number {
  const drop = Math.max(0, winBefore - winAfter);
  const acc = 103.1668 * Math.exp(-0.04354 * drop) - 3.1669;
  return Math.max(0, Math.min(100, acc));
}

/** Classifies a move by centipawn loss for the mover. */
export function classifyMove(cpLoss: number, playedBest: boolean): MoveQuality {
  if (playedBest || cpLoss <= 10) return "best";
  if (cpLoss <= 50) return "good";
  if (cpLoss <= 100) return "inaccuracy";
  if (cpLoss <= 250) return "mistake";
  return "blunder";
}

const QUALITY_TO_CLASS: Record<MoveQuality, MoveClass> = {
  brilliant: "brilliant",
  best: "best",
  good: "great",
  inaccuracy: "inaccuracy",
  mistake: "mistake",
  blunder: "blunder",
};

/**
 * Builds chart plies, classifications and accuracy from per-position evals.
 * `evals[i]` is the evaluation of the position after `i` plies (0 = start),
 * so `evals.length === moves.length + 1`.
 */
export function buildAnalysis(
  sans: string[],
  ucis: string[],
  evals: PositionEval[],
  source: GameAnalysis["source"],
): GameAnalysis {
  const counts: Record<MoveClass, number> = {
    brilliant: 0,
    great: 0,
    best: 0,
    inaccuracy: 0,
    mistake: 0,
    blunder: 0,
  };
  const acc = { w: [] as number[], b: [] as number[] };

  const plies: EvaluatedPly[] = [
    {
      plyIndex: 0,
      evaluation: evals[0] ? evalToCp(evals[0]) : null,
      mate: evals[0]?.mate ?? null,
      bestMove: evals[0]?.bestMove,
    },
  ];

  for (let i = 0; i < sans.length; i++) {
    const before = evals[i];
    const after = evals[i + 1];
    const ply: EvaluatedPly = {
      plyIndex: i + 1,
      san: sans[i],
      evaluation: after ? evalToCp(after) : null,
      mate: after?.mate ?? null,
      bestMove: after?.bestMove,
    };
    if (before && after) {
      const whiteMoved = i % 2 === 0;
      const sign = whiteMoved ? 1 : -1;
      const cpBefore = sign * evalToCp(before);
      const cpAfter = sign * evalToCp(after);
      const cpLoss = Math.max(0, cpBefore - cpAfter);
      const playedBest = !!before.bestMove && before.bestMove === ucis[i];
      ply.quality = classifyMove(cpLoss, playedBest);
      counts[QUALITY_TO_CLASS[ply.quality]]++;
      (whiteMoved ? acc.w : acc.b).push(
        moveAccuracy(winPercent(cpBefore), winPercent(cpAfter)),
      );
    }
    plies.push(ply);
  }

  const mean = (xs: number[]) => (xs.length ? xs.reduce((a, b) => a + b, 0) / xs.length : 0);
  return {
    plies,
    whiteAccuracy: Math.round(mean(acc.w) * 10) / 10,
    blackAccuracy: Math.round(mean(acc.b) * 10) / 10,
    counts,
    source,
  };
}

/** Normalises one backend evaluation entry. */
export function toPositionEval(raw: unknown): PositionEval | null {
  if (raw == null) return null;
  if (typeof raw === "number") return { cp: raw, mate: null };
  if (typeof raw !== "object") return null;
  const r = raw as Record<string, unknown>;
  const n = (v: unknown) => (typeof v === "number" && Number.isFinite(v) ? v : null);
  const cp = n(r.cp) ?? n(r.eval) ?? n(r.evaluation) ?? n(r.centipawns);
  const mate = n(r.mate);
  if (cp === null && mate === null) return null;
  const best = r.bestMove ?? r.best_move ?? r.best;
  return { cp, mate, bestMove: typeof best === "string" ? best : undefined };
}

function movesOf(pgn: string): { sans: string[]; ucis: string[] } | null {
  try {
    const game = new Chess();
    game.loadPgn(pgn);
    const verbose = game.history({ verbose: true });
    return {
      sans: verbose.map((m) => m.san),
      ucis: verbose.map((m) => `${m.from}${m.to}${m.promotion ?? ""}`),
    };
  } catch {
    return null;
  }
}

async function fetchServerEvals(
  gameId: string,
  expected: number,
  signal?: AbortSignal,
): Promise<PositionEval[] | null> {
  try {
    const res = await fetch(endpoints.games.analysis(gameId), {
      headers: { Accept: "application/json" },
      signal,
    });
    if (!res.ok) return null;
    const body = (await res.json()) as unknown;
    const list = Array.isArray(body)
      ? body
      : Array.isArray((body as { evaluations?: unknown })?.evaluations)
        ? (body as { evaluations: unknown[] }).evaluations
        : Array.isArray((body as { plies?: unknown })?.plies)
          ? (body as { plies: unknown[] }).plies
          : null;
    if (!list) return null;
    const evals = list.map(toPositionEval);
    // Accept either per-position (moves + 1) or per-move (moves) lists.
    if (evals.length === expected - 1) evals.unshift({ cp: 20, mate: null });
    if (evals.length !== expected || evals.some((e) => e === null)) return null;
    return evals as PositionEval[];
  } catch {
    return null;
  }
}

/** Analyses each position with the bundled Stockfish worker. */
async function analyseLocally(
  pgn: string,
  depth: number,
  signal?: AbortSignal,
  onProgress?: (done: number, total: number) => void,
): Promise<PositionEval[] | null> {
  if (typeof window === "undefined" || typeof window.Worker === "undefined") return null;
  const game = new Chess();
  try {
    game.loadPgn(pgn);
  } catch {
    return null;
  }
  const history = game.history();
  const replay = new Chess();
  const fens = [replay.fen()];
  for (const san of history) {
    replay.move(san);
    fens.push(replay.fen());
  }

  let worker: Worker;
  try {
    worker = new window.Worker("/assets/stockfish.js");
  } catch {
    return null;
  }

  const evalFen = (fen: string, chess: Chess) =>
    new Promise<PositionEval>((resolve, reject) => {
      // Terminal positions need no search.
      if (chess.isCheckmate()) {
        // Side to move is mated.
        resolve({ cp: chess.turn() === "w" ? -MATE_CP : MATE_CP, mate: null });
        return;
      }
      if (chess.isDraw() || chess.isStalemate()) {
        resolve({ cp: 0, mate: null });
        return;
      }
      const whiteToMove = fen.split(" ")[1] === "w";
      let cp: number | null = null;
      let mate: number | null = null;
      const timeout = setTimeout(() => reject(new Error("Engine timeout")), 15000);
      worker.onmessage = (e: MessageEvent) => {
        const line = typeof e.data === "string" ? e.data : "";
        if (line.startsWith("info") && line.includes(" score ")) {
          const m = / score (cp|mate) (-?\d+)/.exec(line);
          if (m) {
            const v = Number(m[2]);
            if (m[1] === "cp") {
              cp = whiteToMove ? v : -v;
              mate = null;
            } else {
              mate = whiteToMove ? v : -v;
              cp = null;
            }
          }
        } else if (line.startsWith("bestmove")) {
          clearTimeout(timeout);
          resolve({ cp, mate, bestMove: line.split(" ")[1] });
        }
      };
      worker.postMessage(`position fen ${fen}`);
      worker.postMessage(`go depth ${depth}`);
    });

  try {
    worker.postMessage("uci");
    worker.postMessage("ucinewgame");
    const out: PositionEval[] = [];
    for (let i = 0; i < fens.length; i++) {
      if (signal?.aborted) return null;
      out.push(await evalFen(fens[i], new Chess(fens[i])));
      onProgress?.(i + 1, fens.length);
    }
    return out;
  } catch {
    return null;
  } finally {
    worker.terminate();
  }
}

export async function analyseGame(params: {
  gameId: string;
  pgn: string;
  depth?: number;
  signal?: AbortSignal;
  onProgress?: (done: number, total: number) => void;
}): Promise<GameAnalysis | null> {
  const moves = movesOf(params.pgn);
  if (!moves || moves.sans.length === 0) return null;
  const expected = moves.sans.length + 1;

  if (params.gameId && params.gameId !== "unknown") {
    const server = await fetchServerEvals(params.gameId, expected, params.signal);
    if (server) return buildAnalysis(moves.sans, moves.ucis, server, "server");
  }
  if (params.signal?.aborted) return null;

  const local = await analyseLocally(
    params.pgn,
    params.depth ?? 10,
    params.signal,
    params.onProgress,
  );
  if (!local) return null;
  return buildAnalysis(moves.sans, moves.ucis, local, "local");
}
