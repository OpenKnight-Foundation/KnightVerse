/**
 * Guild profiles: `/guilds/[id]` (#FE-108 / #1349).
 *
 * - `GET /v1/guilds/{id}` → guild name, captain and roster.
 *
 * The backend does not expose a guilds API yet, so this module also owns the
 * small preview fixture the page falls back to. Preview guilds are always
 * flagged with `isPreview: true` so the UI can label them and disable actions
 * that would need a real backend.
 */

import { endpoints, fetchWithRetry } from "./api";

export type GuildMemberRole = "Captain" | "Officer" | "Member";

export interface GuildMember {
  /** Integer account id, when the backend sends one. */
  userId?: number;
  username: string;
  rating: number;
  role: GuildMemberRole;
  joinedAt?: string;
}

export interface GuildProfile {
  id: string;
  name: string;
  captainUsername: string | null;
  captainUserId: number | null;
  members: GuildMember[];
  /** 0–100; `undefined` when the backend has no stats for the guild. */
  winRate?: number;
  rank?: number;
  treasuryXlm?: number;
  /** True when this profile is the local sample fixture, not API data. */
  isPreview: boolean;
}

/** The subset of the signed-in account needed to derive guild permissions. */
export interface GuildViewer {
  user_id?: number | null;
  username?: string | null;
}

const num = (v: unknown): number | undefined =>
  typeof v === "number" && Number.isFinite(v) ? v : undefined;

const str = (v: unknown): string | undefined =>
  typeof v === "string" && v.trim().length > 0 ? v.trim() : undefined;

/** Accepts the role spellings the backend is likely to use. */
export function toGuildRole(raw: unknown): GuildMemberRole {
  const role = String(raw ?? "").toLowerCase();
  if (["captain", "leader", "owner", "guild_master", "guildmaster"].includes(role)) {
    return "Captain";
  }
  if (["officer", "admin", "moderator", "mod"].includes(role)) return "Officer";
  return "Member";
}

export function toGuildMember(raw: unknown): GuildMember | null {
  if (!raw || typeof raw !== "object") return null;
  const r = raw as Record<string, unknown>;
  const username = str(r.username) ?? str(r.name) ?? str(r.display_name) ?? str(r.displayName);
  if (!username) return null;

  const userId = num(r.user_id) ?? num(r.userId) ?? num(r.id);
  return {
    userId: userId !== undefined ? Math.trunc(userId) : undefined,
    username,
    rating: Math.round(num(r.rating) ?? num(r.elo) ?? 0),
    role: toGuildRole(r.role ?? r.guild_role),
    joinedAt: str(r.joinedAt) ?? str(r.joined_at) ?? str(r.created_at),
  };
}

interface CaptainRef {
  username: string | null;
  userId: number | null;
}

function toCaptain(r: Record<string, unknown>, members: GuildMember[]): CaptainRef {
  const explicit = r.captain ?? r.owner ?? r.leader;
  if (typeof explicit === "string" && explicit.trim()) {
    return { username: explicit.trim(), userId: null };
  }
  if (explicit && typeof explicit === "object") {
    const c = explicit as Record<string, unknown>;
    const userId = num(c.user_id) ?? num(c.userId) ?? num(c.id);
    return {
      username: str(c.username) ?? str(c.name) ?? null,
      userId: userId !== undefined ? Math.trunc(userId) : null,
    };
  }

  const flatUsername =
    str(r.captain_username) ?? str(r.captainUsername) ?? str(r.captain_name);
  const flatId =
    num(r.captain_user_id) ?? num(r.captainUserId) ?? num(r.captain_id) ?? num(r.captainId);
  if (flatUsername || flatId !== undefined) {
    return {
      username: flatUsername ?? null,
      userId: flatId !== undefined ? Math.trunc(flatId) : null,
    };
  }

  // Fall back to whoever the roster marks as captain.
  const captain = members.find((m) => m.role === "Captain");
  return captain
    ? { username: captain.username, userId: captain.userId ?? null }
    : { username: null, userId: null };
}

/**
 * Normalises a guild payload. Tolerates snake_case/camelCase and either a
 * nested `captain` object or flat `captain_id` / `captain_username` fields.
 */
export function toGuildProfile(
  raw: unknown,
  fallbackId: string,
  isPreview = false,
): GuildProfile {
  const r = (raw && typeof raw === "object" ? raw : {}) as Record<string, unknown>;

  const rawMembers = Array.isArray(r.members)
    ? r.members
    : Array.isArray(r.roster)
      ? r.roster
      : [];
  const members = rawMembers
    .map(toGuildMember)
    .filter((m): m is GuildMember => m !== null);

  const captain = toCaptain(r, members);
  const rawId = r.id ?? r.guild_id;
  const numericId = num(rawId);
  const id =
    str(rawId) ?? (numericId !== undefined ? String(Math.trunc(numericId)) : fallbackId);

  let winRate = num(r.win_rate) ?? num(r.winRate);
  // Accept a 0–1 fraction as well as a percentage.
  if (winRate !== undefined && winRate > 0 && winRate <= 1) winRate *= 100;
  if (winRate !== undefined) winRate = Math.min(100, Math.max(0, winRate));

  return {
    id,
    name: str(r.name) ?? str(r.guild_name) ?? `Guild #${id}`,
    captainUsername: captain.username,
    captainUserId: captain.userId,
    members,
    winRate,
    rank: num(r.rank) ?? num(r.guild_rank),
    treasuryXlm: num(r.treasury) ?? num(r.treasury_xlm) ?? num(r.treasuryXlm),
    isPreview,
  };
}

/** The guild id does not exist (HTTP 404). */
export class GuildNotFoundError extends Error {
  constructor(id: string) {
    super(`Guild "${id}" was not found.`);
    this.name = "GuildNotFoundError";
  }
}

/** The guild API could not be reached at all (network error). */
export class GuildUnavailableError extends Error {
  constructor(message = "The guild service is unreachable.") {
    super(message);
    this.name = "GuildUnavailableError";
  }
}

function authHeaders(): HeadersInit {
  const token =
    typeof window !== "undefined" ? window.localStorage.getItem("access_token") : null;
  return token
    ? { Accept: "application/json", Authorization: `Bearer ${token}` }
    : { Accept: "application/json" };
}

/**
 * Loads one guild. Throws `GuildNotFoundError` for 404s and
 * `GuildUnavailableError` when the request never reached the server; other
 * HTTP failures surface as generic errors so the page can offer a retry.
 */
export async function fetchGuild(id: string, signal?: AbortSignal): Promise<GuildProfile> {
  let res: Response;
  try {
    res = await fetchWithRetry(endpoints.guilds.detail(id), {
      headers: authHeaders(),
      signal,
    });
  } catch (error) {
    // Let aborts propagate untouched so callers can ignore them.
    if ((error as { name?: string } | null)?.name === "AbortError") throw error;
    throw new GuildUnavailableError();
  }

  if (res.status === 404) throw new GuildNotFoundError(id);
  if (!res.ok) throw new Error(`Could not load guild (${res.status})`);

  const body: unknown = await res.json();
  return toGuildProfile(body, id);
}

/**
 * Sample guilds used until the backend ships `/v1/guilds/{id}`. Only the ids
 * linked from the guild directory are covered; anything else stays "not found"
 * unless the API returns it.
 */
const PREVIEW_FIXTURES: GuildProfile[] = [
  {
    id: "1",
    name: "Knights of the Realm",
    captainUsername: "KnightRider",
    captainUserId: null,
    winRate: 62,
    rank: 12,
    treasuryXlm: 12500.5,
    isPreview: true,
    members: [
      { username: "KnightRider", rating: 2100, role: "Captain", joinedAt: "2025-01-10" },
      { username: "PawnStorm", rating: 1850, role: "Officer", joinedAt: "2025-02-04" },
      { username: "BishopBabe", rating: 1720, role: "Member", joinedAt: "2025-03-18" },
    ],
  },
  {
    id: "2",
    name: "Rook & Roll",
    captainUsername: "RookStar",
    captainUserId: null,
    winRate: 55,
    rank: 27,
    treasuryXlm: 8400,
    isPreview: true,
    members: [
      { username: "RookStar", rating: 2020, role: "Captain", joinedAt: "2025-01-22" },
      { username: "QueenOfTrades", rating: 1945, role: "Officer", joinedAt: "2025-02-11" },
      { username: "h4h4h4", rating: 1805, role: "Member", joinedAt: "2025-04-02" },
    ],
  },
  {
    id: "3",
    name: "The En Passant Society",
    captainUsername: "EnPassantPete",
    captainUserId: null,
    winRate: 48,
    rank: 41,
    treasuryXlm: 3200,
    isPreview: true,
    members: [
      { username: "EnPassantPete", rating: 1880, role: "Captain", joinedAt: "2025-02-19" },
      { username: "SicilianSid", rating: 1810, role: "Officer", joinedAt: "2025-03-07" },
      { username: "FianchettoFay", rating: 1675, role: "Member", joinedAt: "2025-05-14" },
    ],
  },
];

/** Returns the sample guild for `id`, or `null` when there is no preview for it. */
export function previewGuild(id: string): GuildProfile | null {
  const fixture = PREVIEW_FIXTURES.find((g) => g.id === id);
  if (!fixture) return null;
  // Return a copy so a consumer can't mutate the fixture.
  return { ...fixture, members: fixture.members.map((m) => ({ ...m })) };
}

function sameViewer(member: { userId?: number; username: string }, viewer: GuildViewer): boolean {
  if (member.userId !== undefined && viewer.user_id != null && member.userId === viewer.user_id) {
    return true;
  }
  return !!viewer.username && member.username.toLowerCase() === viewer.username.toLowerCase();
}

/** True when the signed-in account is the guild's captain. */
export function isGuildCaptain(guild: GuildProfile, viewer: GuildViewer | null): boolean {
  if (!viewer) return false;
  const idMatches =
    guild.captainUserId != null &&
    viewer.user_id != null &&
    guild.captainUserId === viewer.user_id;
  const nameMatches =
    !!guild.captainUsername &&
    !!viewer.username &&
    guild.captainUsername.toLowerCase() === viewer.username.toLowerCase();
  return idMatches || nameMatches;
}

/** True when the signed-in account already appears on the guild roster. */
export function isGuildMember(guild: GuildProfile, viewer: GuildViewer | null): boolean {
  if (!viewer) return false;
  return guild.members.some((member) => sameViewer(member, viewer));
}
