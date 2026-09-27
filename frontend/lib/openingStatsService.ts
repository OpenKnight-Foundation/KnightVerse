/**
 * Master-game statistics service (#1112 FE-53).
 * Hybrid: tries Lichess Masters explorer, falls back to static book stats.
 */

import { statsForEco, type OpeningStats } from "./openings";

export interface ExplorerStats extends OpeningStats {
  source: "lichess-masters" | "fallback";
  fen: string;
}

const CACHE = new Map<string, { at: number; stats: ExplorerStats }>();
const TTL_MS = 10 * 60 * 1000;

export function clearOpeningStatsCache(): void {
  CACHE.clear();
}

async function fetchLichessMasters(fen: string): Promise<OpeningStats | null> {
  try {
    const url = `https://explorer.lichess.ovh/masters?fen=${encodeURIComponent(fen)}`;
    const res = await fetch(url, { headers: { Accept: "application/json" } });
    if (!res.ok) return null;
    const data = (await res.json()) as {
      white?: number;
      draws?: number;
      black?: number;
    };
    if (
      typeof data.white !== "number" ||
      typeof data.draws !== "number" ||
      typeof data.black !== "number"
    ) {
      return null;
    }
    const games = data.white + data.draws + data.black;
    if (games <= 0) return null;
    return { white: data.white, draws: data.draws, black: data.black, games };
  } catch {
    return null;
  }
}

export async function getOpeningStats(
  fen: string,
  eco: string,
): Promise<ExplorerStats> {
  const cached = CACHE.get(fen);
  const now = Date.now();
  if (cached && now - cached.at < TTL_MS) return cached.stats;

  const live = await fetchLichessMasters(fen);
  const fallback = statsForEco(eco);
  const stats: ExplorerStats = live
    ? { ...live, source: "lichess-masters", fen }
    : { ...fallback, source: "fallback", fen };
  CACHE.set(fen, { at: now, stats });
  return stats;
}
