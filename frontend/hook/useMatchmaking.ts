import { useEffect, useRef, useState, useCallback } from "react";
import type { ChessVariant } from "@/lib/chessVariants";

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
  joinMatchmaking: (
    matchType?: "Rated" | "Casual",
    timeControl?: ChessVariant,
  ) => Promise<void>;
  cancelMatchmaking: () => void;
  sendMove: (from: string, to: string, promotion?: string) => void;
  lastOpponentMove: { from: string; to: string; promotion?: string } | null;
}

const API_BASE = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8000";
const WS_BASE = API_BASE.replace(/^http/, "ws");

export function useMatchmaking(): UseMatchmakingReturn {
  const [status, setStatus] = useState<MatchmakingStatus>("idle");
  const [gameId, setGameId] = useState<string | null>(null);
  const [playerColor, setPlayerColor] = useState<"white" | "black" | null>(
    null,
  );
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
      const ws = new WebSocket(`${WS_BASE}/v1/games/${gId}/ws`);
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
    [setStatusSynced],
  );

  const joinMatchmaking = useCallback(
    async (
      matchType: "Rated" | "Casual" = "Casual",
      timeControl: ChessVariant = "standard",
    ) => {
      setStatusSynced("searching");
      setError(null);

      try {
        // wallet_address and elo are resolved server-side from the authenticated session.
        // The server reads the JWT cookie (credentials: "include") to identify the player.
        const res = await fetch(`${API_BASE}/v1/matchmaking/join`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            match_type: matchType,
            time_control: timeControl,
          }),
          credentials: "include",
        });

        if (!res.ok) throw new Error(`Matchmaking failed: ${res.status}`);

        const json = await res.json();
        const sessionId = json.request_id || json.sessionId;
        sessionIdRef.current = sessionId;

        // Open WebSocket to listen for match_found event
        const ws = new WebSocket(
          `${WS_BASE}/v1/matchmaking/ws?session=${sessionId}`,
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
    [openGameSocket, setStatusSynced],
  );

  const cancelMatchmaking = useCallback(() => {
    cleanup();
    if (sessionIdRef.current) {
      // Best-effort cancel; use the correct /cancel endpoint with request_id
      fetch(`${API_BASE}/v1/matchmaking/cancel`, {
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
          JSON.stringify({ type: "move", gameId, from, to, promotion }),
        );
      }
    },
    [gameId],
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
