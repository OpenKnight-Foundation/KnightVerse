"use client";

import React, { createContext, useContext, useState, useEffect, useCallback, useRef } from "react";
import { endpoints, authFetch } from "@/lib/api";

interface AuthUser {
  user_id: number;
  username: string;
  email?: string;
  /** Stellar account linked to this account via the wallet-link flow (#FE-97). */
  wallet_address?: string | null;
}

interface AuthResponse {
  access_token: string;
  refresh_token: string;
  token_type: string;
  expires_in: number;
  refresh_token_expires_in: number;
  user_id: number;
  username: string;
  wallet_address?: string | null;
}

interface SessionInfo {
  session_id: string;
  ip_address?: string;
  user_agent?: string;
  created_at: string;
  expires_at: string;
  last_active_at: string;
}

interface AuthContextType {
  user: AuthUser | null;
  accessToken: string | null;
  isAuthenticated: boolean;
  isLoading: boolean;
  login: (username: string, password: string) => Promise<AuthResponse>;
  register: (username: string, email: string, password: string) => Promise<AuthResponse>;
  logout: () => Promise<void>;
  refreshToken: () => Promise<void>;
  getSessions: () => Promise<SessionInfo[]>;
  revokeSession: (sessionId: string) => Promise<void>;
  revokeAllSessions: () => Promise<void>;
  /** Records the wallet the backend just linked (or unlinked) for this account. */
  setLinkedWallet: (walletAddress: string | null) => void;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

const TOKEN_KEY = "access_token";
const USER_KEY = "user";
/** Refresh this many seconds before the access token expires. */
const REFRESH_MARGIN_SECONDS = 300;
/** Web Locks name that serialises token refresh across tabs (#FE-93). */
const REFRESH_LOCK = "knightverse-auth-refresh";

interface StoredSession {
  token: string;
  user: AuthUser;
}

/**
 * Reads the persisted session. A malformed `user` entry (old format, manual
 * edit, partial write) clears both keys instead of crashing the app (#FE-92).
 */
function readStoredSession(): StoredSession | null {
  try {
    const token = localStorage.getItem(TOKEN_KEY);
    const rawUser = localStorage.getItem(USER_KEY);
    if (!token || !rawUser) return null;
    const user = JSON.parse(rawUser) as AuthUser;
    if (!user || typeof user !== "object") throw new Error("Invalid stored user");
    return { token, user };
  } catch {
    clearStoredSession();
    return null;
  }
}

function persistSession(token: string, user: AuthUser) {
  localStorage.setItem(TOKEN_KEY, token);
  localStorage.setItem(USER_KEY, JSON.stringify(user));
}

function clearStoredSession() {
  try {
    localStorage.removeItem(TOKEN_KEY);
    localStorage.removeItem(USER_KEY);
  } catch {
    // Storage unavailable (e.g. privacy mode): nothing to clear.
  }
}

/** True when the token is expired, undecodable, or inside the refresh margin. */
function needsRefresh(token: string): boolean {
  const exp = decodeJWT(token)?.exp;
  return typeof exp !== "number" || exp - REFRESH_MARGIN_SECONDS <= Date.now() / 1000;
}

/**
 * Runs `fn` while holding a cross-tab lock so only one tab refreshes at a
 * time; tabs that were waiting then adopt the new token from storage.
 */
async function withRefreshLock(fn: () => Promise<void>): Promise<void> {
  const locks = typeof navigator !== "undefined" ? navigator.locks : undefined;
  if (locks?.request) {
    await locks.request(REFRESH_LOCK, fn);
  } else {
    await fn();
  }
}

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [user, setUser] = useState<AuthUser | null>(null);
  const [accessToken, setAccessToken] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const accessTokenRef = useRef<string | null>(null);

  const applySession = useCallback((session: StoredSession | null) => {
    accessTokenRef.current = session?.token ?? null;
    setAccessToken(session?.token ?? null);
    setUser(session?.user ?? null);
  }, []);

  const clearSession = useCallback(() => {
    clearStoredSession();
    applySession(null);
  }, [applySession]);

  // Restore the session on mount. An expired (or nearly expired) token keeps
  // `isLoading` true until the immediate refresh below settles (#FE-91).
  useEffect(() => {
    const stored = readStoredSession();
    if (stored && typeof decodeJWT(stored.token)?.exp !== "number") {
      // A token we can't read can't be refreshed either; start logged out.
      clearSession();
      setIsLoading(false);
      return;
    }
    applySession(stored);
    if (!stored || !needsRefresh(stored.token)) setIsLoading(false);
  }, [applySession, clearSession]);

  const refreshToken = useCallback(async () => {
    const tokenBeforeLock = accessTokenRef.current;
    try {
      await withRefreshLock(async () => {
        const stored = readStoredSession();
        if (!stored) {
          // Another tab logged out (or storage was cleared) while we waited.
          applySession(null);
          return;
        }
        if (stored.token !== tokenBeforeLock && !needsRefresh(stored.token)) {
          // Another tab already refreshed: reuse its token instead of
          // spending the refresh token a second time.
          applySession(stored);
          return;
        }

        const response = await fetch(endpoints.auth.refresh(), {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            Authorization: `Bearer ${stored.token}`,
          },
          credentials: "include",
        });
        if (!response.ok) {
          throw new Error(`Token refresh failed (HTTP ${response.status})`);
        }

        const data = await response.json();
        persistSession(data.access_token, stored.user);
        applySession({ token: data.access_token, user: stored.user });
      });
    } catch (error) {
      console.error("Token refresh error:", error);
      clearSession();
    } finally {
      setIsLoading(false);
    }
  }, [applySession, clearSession]);

  // Refresh REFRESH_MARGIN_SECONDS before expiry, or immediately if the token
  // is already expired or inside the margin (#FE-91).
  useEffect(() => {
    if (!accessToken) return;
    const exp = decodeJWT(accessToken)?.exp;
    if (typeof exp !== "number") return;

    const delay = Math.max(0, (exp - REFRESH_MARGIN_SECONDS) * 1000 - Date.now());
    const timeout = setTimeout(() => {
      void refreshToken();
    }, delay);
    return () => clearTimeout(timeout);
  }, [accessToken, refreshToken]);

  // Mirror login/logout/refresh performed in other tabs (#FE-93). `storage`
  // events only fire in tabs other than the one that wrote the value.
  useEffect(() => {
    const onStorage = (event: StorageEvent) => {
      if (event.key !== null && event.key !== TOKEN_KEY && event.key !== USER_KEY) return;
      applySession(readStoredSession());
    };
    window.addEventListener("storage", onStorage);
    return () => window.removeEventListener("storage", onStorage);
  }, [applySession]);

  const login = useCallback(async (username: string, password: string): Promise<AuthResponse> => {
    const response = await fetch(endpoints.auth.login(), {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ username, password }),
      credentials: "include", // Include cookies for refresh token
    });

    if (!response.ok) {
      const error = await response.json().catch(() => ({}));
      throw new Error(error.message || "Login failed");
    }

    const data: AuthResponse = await response.json();
    const sessionUser = {
      user_id: data.user_id,
      username: data.username,
      wallet_address: data.wallet_address ?? null,
    };
    persistSession(data.access_token, sessionUser);
    applySession({ token: data.access_token, user: sessionUser });

    return data;
  }, [applySession]);

  const register = useCallback(async (username: string, email: string, password: string): Promise<AuthResponse> => {
    const response = await fetch(endpoints.auth.register(), {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ username, email, password }),
      credentials: "include",
    });

    if (!response.ok) {
      const error = await response.json().catch(() => ({}));
      throw new Error(error.message || "Registration failed");
    }

    const data: AuthResponse = await response.json();
    const sessionUser = {
      user_id: data.user_id,
      username: data.username,
      wallet_address: data.wallet_address ?? null,
    };
    persistSession(data.access_token, sessionUser);
    applySession({ token: data.access_token, user: sessionUser });

    return data;
  }, [applySession]);

  const logout = useCallback(async () => {
    try {
      await fetch(endpoints.auth.logout(), {
        method: "POST",
        headers: {
          Authorization: `Bearer ${accessTokenRef.current}`,
        },
        credentials: "include",
      });
    } catch (error) {
      console.error("Logout error:", error);
    } finally {
      clearSession();
    }
  }, [clearSession]);

  const setLinkedWallet = useCallback(
    (walletAddress: string | null) => {
      const stored = readStoredSession();
      if (!stored) return;
      const next = { ...stored, user: { ...stored.user, wallet_address: walletAddress } };
      persistSession(next.token, next.user);
      applySession(next);
    },
    [applySession],
  );

  const getSessions = useCallback(async (): Promise<SessionInfo[]> => {
    const response = await authFetch(
      endpoints.auth.sessions(),
      {},
      { getToken: () => accessTokenRef.current, refresh: refreshToken, logout },
    );

    if (!response.ok) {
      throw new Error("Failed to fetch sessions");
    }

    return response.json();
  }, [refreshToken, logout]);

  const revokeSession = useCallback(async (sessionId: string) => {
    const response = await authFetch(
      endpoints.auth.revokeSession(sessionId),
      { method: "DELETE" },
      { getToken: () => accessTokenRef.current, refresh: refreshToken, logout },
    );

    if (!response.ok) {
      throw new Error("Failed to revoke session");
    }
  }, [refreshToken, logout]);

  const revokeAllSessions = useCallback(async () => {
    const response = await authFetch(
      endpoints.auth.revokeAllSessions(),
      { method: "POST" },
      { getToken: () => accessTokenRef.current, refresh: refreshToken, logout },
    );

    if (!response.ok) {
      throw new Error("Failed to revoke all sessions");
    }

    // Logout from current device as well
    await logout();
  }, [refreshToken, logout]);

  return (
    <AuthContext.Provider
      value={{
        user,
        accessToken,
        isAuthenticated: !!accessToken,
        isLoading,
        login,
        register,
        logout,
        refreshToken,
        getSessions,
        revokeSession,
        revokeAllSessions,
        setLinkedWallet,
      }}
    >
      {children}
    </AuthContext.Provider>
  );
}

export const useAuth = () => {
  const context = useContext(AuthContext);
  if (!context) {
    throw new Error("useAuth must be used within an AuthProvider");
  }
  return context;
};

/**
 * Like `useAuth` but returns `null` instead of throwing when called outside
 * an AuthProvider. Use this for components that optionally sync with the
 * backend when authenticated but work fine without it (e.g. ThemeContext).
 */
export const useAuthOptional = () => useContext(AuthContext) ?? null;

// Helper to decode JWT token (without verification)
function decodeJWT(token: string) {
  try {
    const base64Url = token.split(".")[1];
    const base64 = base64Url.replace(/-/g, "+").replace(/_/g, "/");
    const jsonPayload = decodeURIComponent(
      atob(base64)
        .split("")
        .map((c) => "%" + ("00" + c.charCodeAt(0).toString(16)).slice(-2))
        .join("")
    );
    return JSON.parse(jsonPayload);
  } catch (error) {
    console.error("Failed to decode JWT:", error);
    return null;
  }
}
