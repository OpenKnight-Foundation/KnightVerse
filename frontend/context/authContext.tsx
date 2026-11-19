"use client";

import React, { createContext, useContext, useState, useEffect, useCallback, useRef } from "react";
import { endpoints, authFetch } from "@/lib/api";
import { setSessionAccessToken } from "@/lib/sessionToken";

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

/** Refresh this many seconds before the access token expires. */
const REFRESH_MARGIN_SECONDS = 300;
/** Web Locks name that serialises concurrent refresh calls within this tab. */
const REFRESH_LOCK = "knightverse-auth-refresh";

/**
 * Runs `fn` while holding a lock so only one refresh happens at a time, even
 * if several components ask for one concurrently.
 */
async function withRefreshLock(fn: () => Promise<void>): Promise<void> {
  const locks = typeof navigator !== "undefined" ? navigator.locks : undefined;
  if (locks?.request) {
    await locks.request(REFRESH_LOCK, fn);
  } else {
    await fn();
  }
}

/**
 * Builds the minimal user record we can trust from the access token's own
 * claims (#FE-166). The access token is kept in memory only, so this is the
 * only user data available right after a silent restore; anything else
 * (e.g. a linked wallet) is re-synced once the relevant screen loads.
 */
function userFromToken(token: string): AuthUser | null {
  const claims = decodeJWT(token);
  if (!claims || typeof claims.user_id !== "number" || typeof claims.username !== "string") {
    return null;
  }
  return { user_id: claims.user_id, username: claims.username };
}

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [user, setUser] = useState<AuthUser | null>(null);
  const [accessToken, setAccessToken] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const accessTokenRef = useRef<string | null>(null);
  const userRef = useRef<AuthUser | null>(null);

  const applySession = useCallback((token: string | null, nextUser: AuthUser | null) => {
    accessTokenRef.current = token;
    userRef.current = nextUser;
    setSessionAccessToken(token);
    setAccessToken(token);
    setUser(nextUser);
  }, []);

  const clearSession = useCallback(() => {
    applySession(null, null);
  }, [applySession]);

  const refreshToken = useCallback(async () => {
    try {
      await withRefreshLock(async () => {
        const headers = new Headers({ "Content-Type": "application/json" });
        if (accessTokenRef.current) {
          headers.set("Authorization", `Bearer ${accessTokenRef.current}`);
        }

        const response = await fetch(endpoints.auth.refresh(), {
          method: "POST",
          headers,
          // The refresh token itself lives in an httpOnly cookie; it is never
          // exposed to JavaScript (#FE-166).
          credentials: "include",
        });
        if (!response.ok) {
          throw new Error(`Token refresh failed (HTTP ${response.status})`);
        }

        const data = await response.json();
        const nextUser = userRef.current ?? userFromToken(data.access_token);
        applySession(data.access_token, nextUser);
      });
    } catch (error) {
      console.error("Token refresh error:", error);
      clearSession();
    } finally {
      setIsLoading(false);
    }
  }, [applySession, clearSession]);

  // Restore the session on mount from the httpOnly refresh-token cookie only;
  // the access token is never persisted client-side (#FE-166).
  useEffect(() => {
    void refreshToken();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

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
    applySession(data.access_token, sessionUser);

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
    applySession(data.access_token, sessionUser);

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
      if (!userRef.current) return;
      applySession(accessTokenRef.current, { ...userRef.current, wallet_address: walletAddress });
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
