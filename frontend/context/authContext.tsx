"use client";

import React, { createContext, useContext, useState, useEffect, useCallback } from "react";
import { endpoints } from "@/lib/api";

interface AuthUser {
  user_id: number;
  username: string;
  email?: string;
}

interface AuthResponse {
  access_token: string;
  refresh_token: string;
  token_type: string;
  expires_in: number;
  refresh_token_expires_in: number;
  user_id: number;
  username: string;
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
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [user, setUser] = useState<AuthUser | null>(null);
  const [accessToken, setAccessToken] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  // Initialize auth state from localStorage on mount
  useEffect(() => {
    const storedToken = localStorage.getItem("access_token");
    const storedUser = localStorage.getItem("user");

    if (storedToken && storedUser) {
      setAccessToken(storedToken);
      setUser(JSON.parse(storedUser));
    }
    setIsLoading(false);
  }, []);

  // Auto-refresh token before expiration
  useEffect(() => {
    if (!accessToken) return;

    const tokenData = decodeJWT(accessToken);
    if (!tokenData) return;

    const expiresIn = tokenData.exp - Date.now() / 1000;
    const refreshTime = (expiresIn - 300) * 1000; // Refresh 5 minutes before expiration

    if (refreshTime > 0) {
      const timeout = setTimeout(() => {
        refreshToken();
      }, refreshTime);

      return () => clearTimeout(timeout);
    }
  }, [accessToken]);

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
      const error = await response.json();
      throw new Error(error.message || "Login failed");
    }

    const data: AuthResponse = await response.json();

    setAccessToken(data.access_token);
    setUser({
      user_id: data.user_id,
      username: data.username,
    });

    localStorage.setItem("access_token", data.access_token);
    localStorage.setItem("user", JSON.stringify({
      user_id: data.user_id,
      username: data.username,
    }));

    return data;
  }, []);

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
      const error = await response.json();
      throw new Error(error.message || "Registration failed");
    }

    const data: AuthResponse = await response.json();

    setAccessToken(data.access_token);
    setUser({
      user_id: data.user_id,
      username: data.username,
    });

    localStorage.setItem("access_token", data.access_token);
    localStorage.setItem("user", JSON.stringify({
      user_id: data.user_id,
      username: data.username,
    }));

    return data;
  }, []);

  const logout = useCallback(async () => {
    try {
      await fetch(endpoints.auth.logout(), {
        method: "POST",
        headers: {
          Authorization: `Bearer ${accessToken}`,
        },
        credentials: "include",
      });
    } catch (error) {
      console.error("Logout error:", error);
    } finally {
      setAccessToken(null);
      setUser(null);
      localStorage.removeItem("access_token");
      localStorage.removeItem("user");
    }
  }, [accessToken]);

  const refreshToken = useCallback(async () => {
    try {
      const response = await fetch(endpoints.auth.refresh(), {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Authorization: `Bearer ${accessToken}`,
        },
        credentials: "include",
      });

      if (!response.ok) {
        throw new Error("Token refresh failed");
      }

      const data = await response.json();
      setAccessToken(data.access_token);
      localStorage.setItem("access_token", data.access_token);
    } catch (error) {
      console.error("Token refresh error:", error);
      // If refresh fails, logout user
      await logout();
    }
  }, [accessToken, logout]);

  const getSessions = useCallback(async (): Promise<SessionInfo[]> => {
    const response = await fetch(endpoints.auth.sessions(), {
      headers: {
        Authorization: `Bearer ${accessToken}`,
      },
    });

    if (!response.ok) {
      throw new Error("Failed to fetch sessions");
    }

    return response.json();
  }, [accessToken]);

  const revokeSession = useCallback(async (sessionId: string) => {
    const response = await fetch(endpoints.auth.revokeSession(sessionId), {
      method: "DELETE",
      headers: {
        Authorization: `Bearer ${accessToken}`,
      },
    });

    if (!response.ok) {
      throw new Error("Failed to revoke session");
    }
  }, [accessToken]);

  const revokeAllSessions = useCallback(async () => {
    const response = await fetch(endpoints.auth.revokeAllSessions(), {
      method: "POST",
      headers: {
        Authorization: `Bearer ${accessToken}`,
      },
    });

    if (!response.ok) {
      throw new Error("Failed to revoke all sessions");
    }

    // Logout from current device as well
    await logout();
  }, [accessToken, logout]);

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
