import { useEffect, useRef, useState, useCallback } from "react";
import { API_BASE, WS_BASE, endpoints } from "@/lib/api";

export type MatchmakingStatus =
  | "idle"
  | "searching"
  | "match_found"
  | "connected"
  | "error";

interface MatchFoundPayload {
  gameId: string;
  color: "white" | "black";
  opponentId: string;
}

interface UseMatchmakingReturn {
  status: MatchmakingStatus;
  gameId: string | null;
  playerColor: "white" | "black" | null;
  error: string | null;
  joinMatchmaking: (matchType?: "Rated" | "Casual") => Promise<void>;
  cancelMatchmaking: () => void;
  sendMove: (from: string, to: string, promotion?: string) => void;
  lastOpponentMove: { from: string; to: string; promotion?: string } | null;
}

export function useMatchmaking(): UseMatchmakingReturn {
  const [status, setStatus] = useState<MatchmakingStatus>("idle");
  const [gameId, setGameId] = useState<string | null>(null);
  const [playerColor, setPlayerColor] = useState<"white" | "black" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [lastOpponentMove, setLastOpponentMove] = useState<{
    from: string;
    to: string;
    promotion?: string;
  } | null>(null);

  const matchmakingWsRef = useRef<WebSocket | null>(null);
  const gameWsRef = useRef<WebSocket | null>(null);
  const sessionIdRef = useRef<string | null>(null);

  // Track current status in a ref so WebSocket callbacks always read the latest
  // value without capturing a stale closure — avoids adding `status` to every
  // useCallback dependency array (which would cause infinite re-render loops).
  const statusRef = useRef<MatchmakingStatus>("idle");

  const setStatusSynced = useCallback((next: MatchmakingStatus) => {
    statusRef.current = next;
    setStatus(next);
  }, []);

  const cleanup = useCallback(() => {
    if (matchmakingWsRef.current) {
      matchmakingWsRef.current.close();
      matchmakingWsRef.current = null;
    }
    if (gameWsRef.current) {
      gameWsRef.current.close();
      gameWsRef.current = null;
    }
  }, []);

  const openGameSocket = useCallback(
    (gId: string) => {
      const ws = new WebSocket(endpoints.games.ws(gId));
      gameWsRef.current = ws;

      ws.onopen = () => setStatusSynced("connected");

      ws.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data);
          if (data.type === "move") {
            setLastOpponentMove({
              from: data.from,
              to: data.to,
              promotion: data.promotion,
            });
          }
        } catch {
          // ignore malformed messages
        }
      };

      ws.onerror = () => {
        setError("Game connection error.");
        setStatusSynced("error");
      };

      ws.onclose = () => {
        // Use statusRef to avoid stale closure — reads the value at close time.
        if (statusRef.current === "connected") setStatusSynced("idle");
      };
    },
    [setStatusSynced]
  );

  const joinMatchmaking = useCallback(
    async (matchType: "Rated" | "Casual" = "Casual") => {
      setStatusSynced("searching");
      setError(null);

      try {
        // wallet_address and elo are resolved server-side from the authenticated session.
        // The server reads the JWT cookie (credentials: "include") to identify the player.
        const res = await fetch(endpoints.matchmaking.join(), {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ match_type: matchType }),
          credentials: "include",
        });

        if (!res.ok) throw new Error(`Matchmaking failed: ${res.status}`);

        const json = await res.json();
        const sessionId = json.request_id || json.sessionId;
        sessionIdRef.current = sessionId;

        const ws = new WebSocket(
          endpoints.matchmaking.ws(sessionId)
        );
        matchmakingWsRef.current = ws;

        ws.onmessage = (event) => {
          try {
            const data: { type: string } & Partial<MatchFoundPayload> =
              JSON.parse(event.data);

            if (data.type === "match_found" && data.gameId && data.color) {
              setGameId(data.gameId);
              setPlayerColor(data.color);
              setStatusSynced("match_found");
              ws.close();
              matchmakingWsRef.current = null;
              openGameSocket(data.gameId);
            }
          } catch {
            // ignore malformed messages
          }
        };

        ws.onerror = () => {
          setError("Matchmaking connection error.");
          setStatusSynced("error");
        };

        ws.onclose = () => {
          // Use statusRef.current — not the closed-over `status` state variable —
          // so we always check the actual current status, not a stale snapshot.
          // FE-13: reset to idle so the UI doesn't hang in "searching" forever.
          if (statusRef.current === "searching") {
            setStatusSynced("idle");
            setError("Matchmaking ended without finding a match.");
          }
        };
      } catch (err) {
        setError(err instanceof Error ? err.message : "Unknown error");
        setStatusSynced("error");
      }
    },
    [openGameSocket, setStatusSynced]
  );

  const cancelMatchmaking = useCallback(() => {
    cleanup();
    if (sessionIdRef.current) {
      fetch(endpoints.matchmaking.cancel(), {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ request_id: sessionIdRef.current }),
        credentials: "include",
      }).catch(() => {});
      sessionIdRef.current = null;
    }
    setStatusSynced("idle");
    setGameId(null);
    setPlayerColor(null);
    setError(null);
  }, [cleanup, setStatusSynced]);

  const sendMove = useCallback(
    (from: string, to: string, promotion = "q") => {
      if (gameWsRef.current?.readyState === WebSocket.OPEN && gameId) {
        gameWsRef.current.send(
          JSON.stringify({ type: "move", gameId, from, to, promotion })
        );
      }
    },
    [gameId]
  );

  // Cleanup on unmount
  useEffect(() => () => cleanup(), [cleanup]);

  return {
    status,
    gameId,
    playerColor,
    error,
    joinMatchmaking,
    cancelMatchmaking,
    sendMove,
    lastOpponentMove,
  };
}
