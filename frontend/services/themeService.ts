import type { BoardTheme, ThemeColors } from "@/context/ThemeContext";

export interface ThemePreferencesPayload {
  boardTheme: BoardTheme;
  customPalette?: ThemeColors;
}

/**
 * Callback invoked with a human-readable, non-blocking warning whenever a
 * preference save could not be synced to the backend.
 */
export type ThemeSyncWarning = (message: string) => void;

/** Local-only persistence key. */
export const THEME_PREFERENCES_STORAGE_KEY = "knightverse_theme_preferences";

const SYNC_UNAVAILABLE_WARNING =
  "Theme preferences are saved on this device only. Cross-device sync is unavailable until the backend exposes a preferences endpoint (#FE-104).";

function readLocalPreferences(): ThemePreferencesPayload | null {
  if (typeof window === "undefined") return null;
  try {
    const raw = localStorage.getItem(THEME_PREFERENCES_STORAGE_KEY);
    if (!raw) return null;
    return JSON.parse(raw) as ThemePreferencesPayload;
  } catch (error) {
    console.warn("Could not read local theme preferences:", error);
    return null;
  }
}

function writeLocalPreferences(payload: ThemePreferencesPayload): void {
  if (typeof window === "undefined") return;
  try {
    localStorage.setItem(
      THEME_PREFERENCES_STORAGE_KEY,
      JSON.stringify(payload)
    );
  } catch (error) {
    console.warn("Could not persist theme preferences locally:", error);
  }
}

/**
 * Persist theme preferences for the current user.
 *
 * TODO(#FE-104): the backend has no `profile` scope, so `/v1/profile/theme`
 * and `/v1/profile/preferences` do not exist. Until a real preferences
 * endpoint is paired with a backend issue, preferences are stored client-side
 * only and a non-blocking warning is surfaced so callers can tell the user
 * their choice will not roam between devices. Do NOT reintroduce a profile
 * fetch here without that backend endpoint.
 *
 * Returns `false` while server sync is unavailable (preferences are still
 * saved locally).
 */
export async function syncThemeWithBackend(
  payload: ThemePreferencesPayload,
  token?: string | null,
  onWarning?: ThemeSyncWarning,
): Promise<boolean> {
  void token; // No backend endpoint to authenticate against yet (#FE-104).

  writeLocalPreferences(payload);

  const warning = onWarning ?? ((message: string) => console.warn(message));
  warning(SYNC_UNAVAILABLE_WARNING);

  return false;
}

/**
 * Load the current user's theme preferences.
 *
 * Reads from local storage because the backend preferences route does not
 * exist yet (#FE-104). Returns `null` when nothing has been saved.
 */
export async function fetchThemePreferencesFromBackend(
  token?: string | null,
): Promise<ThemePreferencesPayload | null> {
  void token; // No backend endpoint to authenticate against yet (#FE-104).
  return readLocalPreferences();
}
