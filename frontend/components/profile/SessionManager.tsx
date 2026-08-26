"use client";

import { useState, useEffect, useCallback } from "react";
import { Button } from "@/components/ui/button";
import { useAuth } from "@/context/authContext";

interface Session {
  session_id: string;
  ip_address?: string;
  user_agent?: string;
  created_at: string;
  expires_at: string;
  last_active_at: string;
  is_current?: boolean;
}

export function SessionManager() {
  const { getSessions, revokeSession, revokeAllSessions } = useAuth();
  const [sessions, setSessions] = useState<Session[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadSessions = useCallback(async () => {
    try {
      setIsLoading(true);
      setError(null);
      const sessionsData = await getSessions();
      setSessions(sessionsData);
    } catch (err) {
      console.error("Failed to load sessions:", err);
      setError("Failed to load sessions");
    } finally {
      setIsLoading(false);
    }
  }, [getSessions]);

  useEffect(() => {
    loadSessions();
  }, [loadSessions]);

  const handleRevokeSession = async (sessionId: string) => {
    try {
      await revokeSession(sessionId);
      await loadSessions();
    } catch (err) {
      console.error("Failed to revoke session:", err);
      setError("Failed to revoke session");
    }
  };

  const handleRevokeAllSessions = async () => {
    if (!confirm("Are you sure you want to revoke all sessions? This will log you out from all devices.")) {
      return;
    }

    try {
      await revokeAllSessions();
    } catch (err) {
      console.error("Failed to revoke all sessions:", err);
      setError("Failed to revoke all sessions");
    }
  };

  const formatUserAgent = (userAgent?: string) => {
    if (!userAgent) return "Unknown device";
    
    // Simple parsing for common browsers
    if (userAgent.includes("Chrome")) return "Chrome";
    if (userAgent.includes("Firefox")) return "Firefox";
    if (userAgent.includes("Safari")) return "Safari";
    if (userAgent.includes("Edge")) return "Edge";
    return "Other browser";
  };

  const formatDate = (dateString: string) => {
    return new Date(dateString).toLocaleString();
  };

  if (isLoading) {
    return (
      <div className="p-4 bg-gray-800/50 border border-gray-700 rounded-lg">
        <p className="text-center text-gray-400">Loading sessions...</p>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="text-lg font-semibold text-white">Active Sessions</h3>
        <Button
          onClick={handleRevokeAllSessions}
          variant="outline"
          size="sm"
          className="text-red-400 border-red-800 hover:bg-red-900/20"
        >
          Revoke All Sessions
        </Button>
      </div>

      {error && (
        <div className="p-3 bg-red-900/20 border border-red-800 rounded-lg">
          <p className="text-sm text-red-400">{error}</p>
        </div>
      )}

      <div className="space-y-2">
        {sessions.length === 0 ? (
          <p className="text-center text-gray-400 py-4">No active sessions</p>
        ) : (
          sessions.map((session) => (
            <div
              key={session.session_id}
              className={`p-4 border rounded-lg ${
                session.is_current
                  ? "bg-emerald-900/20 border-emerald-700"
                  : "bg-gray-800/50 border-gray-700"
              }`}
            >
              <div className="flex items-start justify-between">
                <div className="flex-1">
                  <div className="flex items-center gap-2 mb-2">
                    <div className="w-2 h-2 rounded-full bg-emerald-400" />
                    <span className="text-sm font-medium text-white">
                      {formatUserAgent(session.user_agent)}
                    </span>
                    {session.is_current && (
                      <span className="text-xs px-2 py-0.5 bg-emerald-900/50 text-emerald-400 rounded-full">
                        Current
                      </span>
                    )}
                  </div>

                  <div className="space-y-1 text-xs text-gray-400">
                    {session.ip_address && (
                      <div>
                        <span className="text-gray-500">IP:</span> {session.ip_address}
                      </div>
                    )}
                    <div>
                      <span className="text-gray-500">Created:</span>{" "}
                      {formatDate(session.created_at)}
                    </div>
                    <div>
                      <span className="text-gray-500">Last Active:</span>{" "}
                      {formatDate(session.last_active_at)}
                    </div>
                    <div>
                      <span className="text-gray-500">Expires:</span>{" "}
                      {formatDate(session.expires_at)}
                    </div>
                  </div>
                </div>

                {!session.is_current && (
                  <Button
                    onClick={() => handleRevokeSession(session.session_id)}
                    variant="outline"
                    size="sm"
                    className="text-red-400 border-red-800 hover:bg-red-900/20 ml-4"
                  >
                    Revoke
                  </Button>
                )}
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
