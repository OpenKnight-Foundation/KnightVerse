import { endpoints } from "@/lib/api";

export type PlayerPreferences = Record<string, unknown>;

function currentToken(token?: string | null): string | null {
  if (token) return token;
  if (typeof window === "undefined") return null;
  try {
    return localStorage.getItem("access_token");
  } catch {
    return null;
  }
}

function parsePreferences(value: unknown): PlayerPreferences | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  const preferences = (value as { data?: { preferences?: unknown } }).data?.preferences;
  if (!preferences || typeof preferences !== "object" || Array.isArray(preferences)) {
    return null;
  }
  return preferences as PlayerPreferences;
}

export async function fetchPlayerPreferences(
  token?: string | null,
): Promise<PlayerPreferences | null> {
  const authToken = currentToken(token);
  if (!authToken) return null;
  const response = await fetch(endpoints.players.preferences(), {
    headers: { Authorization: `Bearer ${authToken}` },
    cache: "no-store",
  });
  if (!response.ok) {
    throw new Error(`Preference fetch failed (HTTP ${response.status})`);
  }
  return parsePreferences(await response.json());
}

export async function savePlayerPreferences(
  updates: PlayerPreferences,
  token?: string | null,
): Promise<boolean> {
  const authToken = currentToken(token);
  if (!authToken) return false;
  const response = await fetch(endpoints.players.preferences(), {
    method: "PUT",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${authToken}`,
    },
    body: JSON.stringify(updates),
  });
  if (!response.ok) {
    throw new Error(`Preference sync failed (HTTP ${response.status})`);
  }
  return true;
}
