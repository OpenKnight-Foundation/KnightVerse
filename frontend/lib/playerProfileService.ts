/**
 * Public player profiles: `/player/[username]`.
 *
 * - `GET /v1/players/{username}` → ratings per time control, titles.
 * - `GET /v1/players/{username}/games?limit=N` → recent games.
 * - `GET /v1/players/{username}/elo-history` → rating history (EloRatingChart).
 */

import { endpoints } from "./api";
import type { EloDataPoint } from "@/components/profile/EloRatingChart";

export interface PlayerProfile {
  username: string;
  /** Rating per time control, e.g. { blitz: 1650 }. */
  ratings: Record<string, number>;
  titles: string[];
  joinedAt?: string;
  walletAddress?: string | null;
}

export type GameResultForPlayer = "win" | "loss" | "draw";

export interface RecentGame {
  id: string;
  opponent: string;
  result: GameResultForPlayer;
  color?: "white" | "black";
  ratingChange?: number;
  timeControl?: string;
  playedAt?: string;
}

const num = (v: unknown) => (typeof v === "number" && Number.isFinite(v) ? v : undefined);
const str = (v: unknown) => (typeof v === "string" && v.length > 0 ? v : undefined);

export function toPlayerProfile(raw: unknown, fallbackUsername: string): PlayerProfile {
  const r = (raw && typeof raw === "object" ? raw : {}) as Record<string, unknown>;
  const ratings: Record<string, number> = {};
  if (r.ratings && typeof r.ratings === "object") {
    for (const [k, v] of Object.entries(r.ratings as Record<string, unknown>)) {
      const n = num(v) ?? num((v as { rating?: unknown })?.rating);
      if (n !== undefined) ratings[k] = Math.round(n);
    }
  }
  const single = num(r.rating) ?? num(r.elo);
  if (single !== undefined && Object.keys(ratings).length === 0) ratings.overall = Math.round(single);

  const titles: string[] = Array.isArray(r.titles)
    ? r.titles.filter((t): t is string => typeof t === "string")
    : str(r.title)
      ? [r.title as string]
      : [];

  return {
    username: str(r.username) ?? str(r.name) ?? fallbackUsername,
    ratings,
    titles,
    joinedAt: str(r.joinedAt) ?? str(r.created_at) ?? str(r.joined_at),
    walletAddress: str(r.wallet_address) ?? str(r.walletAddress) ?? null,
  };
}

function toResult(v: unknown): GameResultForPlayer | null {
  const s = String(v ?? "").toLowerCase();
  if (["win", "won", "w", "1"].includes(s)) return "win";
  if (["loss", "lose", "lost", "l", "0"].includes(s)) return "loss";
  if (["draw", "d", "½", "0.5", "1/2"].includes(s)) return "draw";
  return null;
}

export function toRecentGame(raw: unknown): RecentGame | null {
  if (!raw || typeof raw !== "object") return null;
  const r = raw as Record<string, unknown>;
  const id = str(r.id) ?? (num(r.id) !== undefined ? String(r.id) : undefined) ?? str(r.game_id);
  const opponent = str(r.opponent) ?? str(r.opponent_username) ?? str(r.opponentName);
  const result = toResult(r.result ?? r.outcome);
  if (!id || !opponent || !result) return null;
  const color = r.color === "white" || r.color === "black" ? r.color : undefined;
  return {
    id,
    opponent,
    result,
    color,
    ratingChange: num(r.ratingChange) ?? num(r.rating_change) ?? num(r.elo_change),
    timeControl: str(r.timeControl) ?? str(r.time_control),
    playedAt: str(r.playedAt) ?? str(r.played_at) ?? str(r.created_at),
  };
}

function authHeaders(): HeadersInit {
  const token =
    typeof window !== "undefined" ? window.localStorage.getItem("access_token") : null;
  return token
    ? { Accept: "application/json", Authorization: `Bearer ${token}` }
    : { Accept: "application/json" };
}

export class PlayerNotFoundError extends Error {
  constructor(username: string) {
    super(`Player "${username}" was not found.`);
    this.name = "PlayerNotFoundError";
  }
}

export async function fetchPlayerProfile(username: string, signal?: AbortSignal) {
  const res = await fetch(endpoints.players.profile(username), { headers: authHeaders(), signal });
  if (res.status === 404) throw new PlayerNotFoundError(username);
  if (!res.ok) throw new Error(`Could not load player (${res.status})`);
  return toPlayerProfile(await res.json(), username);
}

/** Recent games; missing endpoint degrades to an empty list. */
export async function fetchRecentGames(username: string, signal?: AbortSignal) {
  const res = await fetch(endpoints.players.games(username, 10), { headers: authHeaders(), signal });
  if (res.status === 404) return [] as RecentGame[];
  if (!res.ok) throw new Error(`Could not load recent games (${res.status})`);
  const body: unknown = await res.json();
  const list = Array.isArray(body)
    ? body
    : Array.isArray((body as { games?: unknown })?.games)
      ? (body as { games: unknown[] }).games
      : [];
  return list.map(toRecentGame).filter((g): g is RecentGame => g !== null);
}

/** Rating history; missing endpoint degrades to an empty list (#FE-103). */
export async function fetchEloHistory(username: string, signal?: AbortSignal) {
  const res = await fetch(endpoints.players.eloHistory(username), { headers: authHeaders(), signal });
  if (res.status === 404) return [] as EloDataPoint[];
  if (!res.ok) throw new Error(`Could not load rating history (${res.status})`);
  const body: unknown = await res.json();
  return Array.isArray(body) ? (body as EloDataPoint[]) : [];
}

/** In-app URL for a player's public profile. */
export function playerHref(username: string): string {
  return `/player/${encodeURIComponent(username)}`;
}
