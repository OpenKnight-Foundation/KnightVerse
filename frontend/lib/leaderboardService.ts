/**
 * Leaderboard data (BE-158). Normalises the backend response into the rows
 * rendered by `LeaderboardTable`.
 */

import { endpoints, fetchWithRetry } from "./api";
import type { LeaderboardRow, LeaderboardTier } from "@/app/dashboard/LeaderboardTable";

export type LeaderboardTimeControl = "all" | "bullet" | "blitz" | "rapid" | "classical";

export interface LeaderboardPage {
  rows: LeaderboardRow[];
  total: number;
  page: number;
  pageSize: number;
}

const TIERS: LeaderboardTier[] = ["Grandmaster", "Master", "Diamond", "Gold", "Silver", "Bronze"];

/** Fallback tier when the backend doesn't send one. */
export function tierForRating(rating: number): LeaderboardTier {
  if (rating >= 2400) return "Grandmaster";
  if (rating >= 2200) return "Master";
  if (rating >= 2000) return "Diamond";
  if (rating >= 1700) return "Gold";
  if (rating >= 1400) return "Silver";
  return "Bronze";
}

const num = (v: unknown): number | undefined =>
  typeof v === "number" && Number.isFinite(v) ? v : undefined;

export function toLeaderboardRow(raw: unknown, fallbackRank: number): LeaderboardRow | null {
  if (!raw || typeof raw !== "object") return null;
  const r = raw as Record<string, unknown>;
  const player =
    [r.player, r.username, r.display_name, r.name].find(
      (v): v is string => typeof v === "string" && v.length > 0,
    ) ?? null;
  const rating = num(r.rating) ?? num(r.elo);
  if (!player || rating === undefined) return null;

  const wins = num(r.wins);
  const games = num(r.games) ?? num(r.games_played);
  let winRate = num(r.winRate) ?? num(r.win_rate);
  if (winRate === undefined && wins !== undefined && games) winRate = (wins / games) * 100;
  // Accept 0–1 fractions as well as percentages.
  if (winRate !== undefined && winRate <= 1 && winRate > 0) winRate *= 100;

  const rawTier = typeof r.tier === "string" ? r.tier : "";
  const tier =
    TIERS.find((t) => t.toLowerCase() === rawTier.toLowerCase()) ?? tierForRating(rating);

  return {
    rank: num(r.rank) ?? fallbackRank,
    player,
    tier,
    rating: Math.round(rating),
    winRate: Math.round(winRate ?? 0),
  };
}

export async function fetchLeaderboard(params: {
  timeControl: LeaderboardTimeControl;
  page: number;
  pageSize: number;
  signal?: AbortSignal;
}): Promise<LeaderboardPage> {
  const res = await fetchWithRetry(
    endpoints.leaderboard.list({
      timeControl: params.timeControl === "all" ? undefined : params.timeControl,
      page: params.page,
      pageSize: params.pageSize,
    }),
    { headers: { Accept: "application/json" }, signal: params.signal },
  );
  if (!res.ok) throw new Error(`Leaderboard request failed (${res.status})`);
  const body: unknown = await res.json();

  const list: unknown[] = Array.isArray(body)
    ? body
    : Array.isArray((body as { entries?: unknown })?.entries)
      ? (body as { entries: unknown[] }).entries
      : Array.isArray((body as { players?: unknown })?.players)
        ? (body as { players: unknown[] }).players
        : Array.isArray((body as { data?: unknown })?.data)
          ? (body as { data: unknown[] }).data
          : [];

  const offset = (params.page - 1) * params.pageSize;
  const rows = list
    .map((r, i) => toLeaderboardRow(r, offset + i + 1))
    .filter((r): r is LeaderboardRow => r !== null);
  const total = num((body as { total?: unknown })?.total) ?? offset + rows.length;
  return { rows, total, page: params.page, pageSize: params.pageSize };
}
