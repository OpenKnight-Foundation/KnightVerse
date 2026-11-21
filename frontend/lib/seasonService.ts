import { endpoints, fetchWithRetry } from "./api";

export interface SeasonStatus {
  seasonId: string;
  name: string;
  phase: "active" | "ended";
  prizePool: number;
  currency: string;
  endsAt?: string;
  claimDeadline?: string;
}

export interface SeasonStanding {
  rank: number;
  player: string;
  points: number;
  prize?: number;
}

export interface SeasonClaim {
  seasonId: string;
  amount: number;
  currency: string;
  proof: string[];
}

export interface SeasonClaimResult {
  claimed: boolean;
  amount: number;
  transactionHash?: string | null;
}

const asRecord = (value: unknown): Record<string, unknown> =>
  value && typeof value === "object" ? (value as Record<string, unknown>) : {};

const asNumber = (value: unknown): number =>
  typeof value === "number" && Number.isFinite(value) ? value : 0;

const asString = (value: unknown): string => (typeof value === "string" ? value : "");

function withAuth(headers: Record<string, string>): Record<string, string> {
  const token =
    typeof window !== "undefined" ? window.localStorage.getItem("access_token") : null;
  return token ? { ...headers, Authorization: `Bearer ${token}` } : headers;
}

async function getJson(url: string, signal?: AbortSignal): Promise<unknown> {
  const response = await fetchWithRetry(url, {
    headers: withAuth({ Accept: "application/json" }),
    signal,
  });
  if (!response.ok) throw new Error(`Season request failed (${response.status})`);
  return response.json();
}

export async function fetchSeasonStatus(signal?: AbortSignal): Promise<SeasonStatus> {
  const raw = asRecord(await getJson(endpoints.season.status(), signal));
  const phase = raw.phase === "ended" ? "ended" : "active";
  return {
    seasonId: asString(raw.season_id) || "current",
    name: asString(raw.name) || "Current Season",
    phase,
    prizePool: asNumber(raw.prize_pool),
    currency: asString(raw.currency) || "XLM",
    endsAt: asString(raw.ends_at) || undefined,
    claimDeadline: asString(raw.claim_deadline) || undefined,
  };
}

export async function fetchSeasonStandings(signal?: AbortSignal): Promise<SeasonStanding[]> {
  const raw = await getJson(endpoints.season.standings(), signal);
  const entries = Array.isArray(raw)
    ? raw
    : Array.isArray((raw as { entries?: unknown })?.entries)
      ? (raw as { entries: unknown[] }).entries
      : [];
  return entries.map((entry, index) => {
    const row = asRecord(entry);
    return {
      rank: asNumber(row.rank) || index + 1,
      player: asString(row.player || row.username),
      points: asNumber(row.points),
      prize: typeof row.prize === "number" ? row.prize : undefined,
    };
  }).filter((row) => row.player.length > 0);
}

export async function fetchSeasonClaim(signal?: AbortSignal): Promise<SeasonClaim | null> {
  const raw = await getJson(endpoints.season.claimable(), signal);
  const amount = asNumber((raw as { amount?: unknown })?.amount);
  const proof = Array.isArray((raw as { proof?: unknown })?.proof)
    ? ((raw as { proof: unknown[] }).proof.filter((item): item is string => typeof item === "string"))
    : [];
  if (amount <= 0 || proof.length === 0) return null;
  return {
    seasonId: asString((raw as { season_id?: unknown })?.season_id) || "current",
    amount,
    currency: asString((raw as { currency?: unknown })?.currency) || "XLM",
    proof,
  };
}

export async function claimSeasonPrize(claim: SeasonClaim): Promise<SeasonClaimResult> {
  const response = await fetchWithRetry(endpoints.season.claim(), {
    method: "POST",
    headers: withAuth({ "Content-Type": "application/json" }),
    body: JSON.stringify({
      season_id: claim.seasonId,
      amount: claim.amount,
      proof: claim.proof,
    }),
  });
  if (!response.ok) throw new Error(`Prize claim failed (${response.status})`);
  const raw = asRecord(await response.json());
  return {
    claimed: Boolean(raw.claimed),
    amount: asNumber(raw.amount),
    transactionHash: typeof raw.transaction_hash === "string" ? raw.transaction_hash : null,
  };
}
