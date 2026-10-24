import {
  fetchPlayerPreferences,
  savePlayerPreferences,
} from "@/services/playerPreferencesService";
import type { AuthFetchDeps } from "@/lib/api";
import type { BoardTheme, ThemeColors } from "@/context/ThemeContext";

export interface ThemePreferencesPayload {
  boardTheme: BoardTheme;
  customPalette?: ThemeColors;
}

export type ThemeSyncWarning = (message: string) => void;

export const THEME_PREFERENCES_STORAGE_KEY = "knightverse_theme_preferences";

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
    localStorage.setItem(THEME_PREFERENCES_STORAGE_KEY, JSON.stringify(payload));
  } catch (error) {
    console.warn("Could not persist theme preferences locally:", error);
  }
}

export async function syncThemeWithBackend(
  payload: ThemePreferencesPayload,
  deps: AuthFetchDeps,
  onWarning?: ThemeSyncWarning,
): Promise<boolean> {
  writeLocalPreferences(payload);

  try {
    return await savePlayerPreferences({ appearance: payload }, deps);
  } catch (error) {
    const warning = onWarning ?? ((message: string) => console.warn(message));
    warning(`Theme preferences are saved on this device only. ${String(error)}`);
    return false;
  }
}

export async function fetchThemePreferencesFromBackend(
  deps: AuthFetchDeps,
): Promise<ThemePreferencesPayload | null> {
  try {
    const preferences = await fetchPlayerPreferences(deps);
    const appearance = preferences?.appearance;
    if (
      appearance &&
      typeof appearance === "object" &&
      !Array.isArray(appearance) &&
      Object.keys(appearance).length > 0
    ) {
      const theme = appearance as ThemePreferencesPayload;
      writeLocalPreferences(theme);
      return theme;
    }
    return readLocalPreferences();
  } catch (error) {
    console.warn("Could not load theme preferences from the backend:", error);
    return readLocalPreferences();
  }
}
