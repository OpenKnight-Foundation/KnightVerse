/**
 * Next-move statistics for an opening position (BE-178).
 *
 * Tries the KnightVerse backend explorer first, then falls back to the public
 * Lichess Masters explorer (already used by `openingStatsService`). Results
 * are cached per FEN.
 */

import { endpoints } from "./api";

export interface MoveStat {
  san: string;
  uci?: string;
  white: number;
  draws: number;
  black: number;
  games: number;
}

export interface PositionMoveStats {
  fen: string;
  moves: MoveStat[];
  source: "knightverse" | "lichess-masters" | "none";
}

const CACHE = new Map<string, { at: number; stats: PositionMoveStats }>();
const TTL_MS = 10 * 60 * 1000;

export function clearOpeningMoveStatsCache(): void {
  CACHE.clear();
}

const num = (v: unknown) => (typeof v === "number" && Number.isFinite(v) ? v : 0);

/** Normalises a raw move entry from either API into a `MoveStat`. */
export function toMoveStat(raw: unknown): MoveStat | null {
  if (!raw || typeof raw !== "object") return null;
  const r = raw as Record<string, unknown>;
  const san = typeof r.san === "string" ? r.san : typeof r.move === "string" ? r.move : null;
  if (!san) return null;
  const white = num(r.white ?? r.wins);
  const draws = num(r.draws);
  const black = num(r.black ?? r.losses);
  const games = num(r.games) || white + draws + black;
  if (games <= 0) return null;
  return {
    san,
    uci: typeof r.uci === "string" ? r.uci : undefined,
    white,
    draws,
    black,
    games,
  };
}

function parseMoves(data: unknown): MoveStat[] | null {
  if (!data || typeof data !== "object") return null;
  const moves = (data as { moves?: unknown }).moves;
  if (!Array.isArray(moves)) return null;
  return moves
    .map(toMoveStat)
    .filter((m): m is MoveStat => m !== null)
    .sort((a, b) => b.games - a.games);
}

async function fetchJson(url: string, signal?: AbortSignal): Promise<unknown | null> {
  try {
    const res = await fetch(url, { headers: { Accept: "application/json" }, signal });
    if (!res.ok) return null;
    return await res.json();
  } catch {
    return null;
  }
}

export async function getPositionMoveStats(
  fen: string,
  signal?: AbortSignal,
): Promise<PositionMoveStats> {
  const cached = CACHE.get(fen);
  const now = Date.now();
  if (cached && now - cached.at < TTL_MS) return cached.stats;

  let stats: PositionMoveStats = { fen, moves: [], source: "none" };

  const own = parseMoves(await fetchJson(endpoints.openings.explorer(fen), signal));
  if (own && own.length > 0) {
    stats = { fen, moves: own, source: "knightverse" };
  } else {
    const lichess = parseMoves(
      await fetchJson(
        `https://explorer.lichess.ovh/masters?fen=${encodeURIComponent(fen)}`,
        signal,
      ),
    );
    if (lichess && lichess.length > 0) {
      stats = { fen, moves: lichess, source: "lichess-masters" };
    }
  }

  if (signal?.aborted) return stats;
  if (stats.source !== "none") CACHE.set(fen, { at: now, stats });
  return stats;
}
