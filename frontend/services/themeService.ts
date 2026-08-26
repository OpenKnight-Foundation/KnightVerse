import { endpoints } from "@/lib/api";
import type { BoardTheme, ThemeColors } from "@/context/ThemeContext";

export interface ThemePreferencesPayload {
  boardTheme: BoardTheme;
  customPalette?: ThemeColors;
}

/**
 * Sync user theme preferences with backend profile API when authenticated.
 * Fails gracefully and silently logs errors if server is unreachable or offline.
 */
export async function syncThemeWithBackend(
  payload: ThemePreferencesPayload,
  token?: string | null
): Promise<boolean> {
  const authToken =
    token ??
    (typeof window !== "undefined"
      ? localStorage.getItem("access_token")
      : null);
  if (!authToken) {
    return false;
  }

  try {
    const response = await fetch(endpoints.profile.theme(), {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${authToken}`,
      },
      body: JSON.stringify(payload),
    });

    return response.ok;
  } catch (error) {
    // Graceful fallback for offline / mock server environments
    console.warn("Could not sync theme with backend:", error);
    return false;
  }
}

/**
 * Fetch user theme preferences from backend profile API when authenticated.
 */
export async function fetchThemePreferencesFromBackend(
  token?: string | null
): Promise<ThemePreferencesPayload | null> {
  const authToken =
    token ??
    (typeof window !== "undefined"
      ? localStorage.getItem("access_token")
      : null);
  if (!authToken) {
    return null;
  }

  try {
    const response = await fetch(endpoints.profile.preferences(), {
      method: "GET",
      headers: {
        Authorization: `Bearer ${authToken}`,
      },
    });

    if (!response.ok) {
      return null;
    }

    const data = await response.json();
    if (data && data.boardTheme) {
      return {
        boardTheme: data.boardTheme,
        customPalette: data.customPalette,
      };
    }
    return null;
  } catch (error) {
    console.warn("Could not fetch theme preferences from backend:", error);
    return null;
  }
}
