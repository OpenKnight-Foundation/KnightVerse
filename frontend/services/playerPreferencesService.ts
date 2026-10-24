import { endpoints, authFetch, type AuthFetchDeps } from "@/lib/api";

export type PlayerPreferences = Record<string, unknown>;

function parsePreferences(value: unknown): PlayerPreferences | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  const preferences = (value as { data?: { preferences?: unknown } }).data?.preferences;
  if (!preferences || typeof preferences !== "object" || Array.isArray(preferences)) {
    return null;
  }
  return preferences as PlayerPreferences;
}

export async function fetchPlayerPreferences(
  deps: AuthFetchDeps,
): Promise<PlayerPreferences | null> {
  if (!deps.getToken()) return null;
  const response = await authFetch(
    endpoints.players.preferences(),
    { cache: "no-store" },
    deps,
  );
  if (!response.ok) {
    throw new Error(`Preference fetch failed (HTTP ${response.status})`);
  }
  return parsePreferences(await response.json());
}

export async function savePlayerPreferences(
  updates: PlayerPreferences,
  deps: AuthFetchDeps,
): Promise<boolean> {
  if (!deps.getToken()) return false;
  const response = await authFetch(
    endpoints.players.preferences(),
    {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(updates),
    },
    deps,
  );
  if (!response.ok) {
    throw new Error(`Preference sync failed (HTTP ${response.status})`);
  }
  return true;
}
