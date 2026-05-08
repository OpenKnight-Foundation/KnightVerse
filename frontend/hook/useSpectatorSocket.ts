"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { Chess } from "chess.js";
import {
  createMockSpectatorGameState,
  getMockLiveGameById,
} from "@/constants/mockLiveGames";

export type SpectatorStatus =
  | "idle"
  | "connecting"
  | "connected"
  | "reconnecting"
  | "disconnected"
  | "error";

export interface SpectatorMove {
  from: string;
  to: string;
  san: string;
  color: "w" | "b";
  promotion?: string;
}

export interface SpectatorGameState {
  fen: string;
  moves: SpectatorMove[];
  whiteTime: number;
  blackTime: number;
  status: "playing" | "checkmate" | "stalemate" | "draw" | "resigned";
  spectatorCount: number;
  white: { address: string; elo: number };
  black: { address: string; elo: number };
  result?: "1-0" | "0-1" | "1/2-1/2";
}

interface UseSpectatorSocketReturn {
  status: SpectatorStatus;
  gameState: SpectatorGameState | null;
  disconnect: () => void;
  reconnect: () => void;
}

const API_BASE = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8000";
const WS_BASE = API_BASE.replace(/^http/, "ws");
const MAX_RECONNECT_ATTEMPTS = 10;
const INITIAL_RECONNECT_DELAY = 1000;
const MAX_RECONNECT_DELAY = 30000;
const RECONNECT_TIMEOUT = 3000;

function normalizeGameOverReason(reason: string): SpectatorGameState["status"] {
  const normalizedReason = reason.toLowerCase();

  if (normalizedReason.includes("mate")) {
    return "checkmate";
  }

  if (normalizedReason.includes("stale")) {
    return "stalemate";
  }

  if (normalizedReason.includes("draw")) {
    return "draw";
  }

  if (normalizedReason.includes("resign")) {
    return "resigned";
  }

  return "draw";
}

export function useSpectatorSocket(gameId: string | null): UseSpectatorSocketReturn {
  const [status, setStatus] = useState<SpectatorStatus>("idle");
  const [gameState, setGameState] = useState<SpectatorGameState | null>(null);

  const wsRef = useRef<WebSocket | null>(null);
  const reconnectAttemptsRef = useRef(0);
  const reconnectTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const reconnectTimerRef = useRef<NodeJS.Timeout | null>(null);
  const isManualDisconnectRef = useRef(false);
  const isOnlineRef = useRef(typeof navigator !== "undefined" ? navigator.onLine : true);
  const chessRef = useRef(new Chess());
  const usingMockDataRef = useRef(false);

  const clearReconnectTimeout = useCallback(() => {
    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current);
      reconnectTimeoutRef.current = null;
    }

    if (reconnectTimerRef.current) {
      clearTimeout(reconnectTimerRef.current);
      reconnectTimerRef.current = null;
    }
  }, []);

  const calculateReconnectDelay = useCallback((attempt: number): number => {
    const baseDelay = INITIAL_RECONNECT_DELAY * Math.pow(2, attempt);
    const jitter = Math.random() * 0.3 * baseDelay;
    return Math.min(baseDelay + jitter, MAX_RECONNECT_DELAY);
  }, []);

  const bootstrapMockState = useCallback((nextGameId: string) => {
    usingMockDataRef.current = true;
    chessRef.current = new Chess();

    const mockState = createMockSpectatorGameState(nextGameId);
    try {
      chessRef.current.load(mockState.fen);
    } catch {
      chessRef.current.reset();
    }

    setGameState(mockState);
    setStatus("connected");
  }, []);

  const createWebSocket = useCallback(
    (attemptReconnect = false): WebSocket | null => {
      if (!gameId) {
        return null;
      }

      if (getMockLiveGameById(gameId)) {
        bootstrapMockState(gameId);
        return null;
      }

      try {
        const ws = new WebSocket(`${WS_BASE}/v1/games/${gameId}/spectate`);
        wsRef.current = ws;
        usingMockDataRef.current = false;
        setStatus(attemptReconnect ? "reconnecting" : "connecting");

        ws.onopen = () => {
          setStatus("connected");
          reconnectAttemptsRef.current = 0;
          ws.send(JSON.stringify({ type: "sync", gameId }));
        };

        ws.onmessage = (event) => {
          try {
            const data = JSON.parse(event.data) as
              | (SpectatorGameState & { type: "sync" })
              | ({ type: "move"; fen?: string } & SpectatorMove)
              | { type: "clock"; whiteTime: number; blackTime: number }
              | { type: "spectator_count"; count: number }
              | { type: "game_over"; result: "1-0" | "0-1" | "1/2-1/2"; reason: string };

            switch (data.type) {
              case "sync": {
                chessRef.current = new Chess();
                try {
                  chessRef.current.load(data.fen);
                } catch {
                  chessRef.current.reset();
                  for (const move of data.moves) {
                    try {
                      chessRef.current.move({
                        from: move.from,
                        to: move.to,
                        promotion: move.promotion ?? "q",
                      });
                    } catch {
                      break;
                    }
                  }
                }
                setGameState({
                  fen: data.fen,
                  moves: data.moves,
                  whiteTime: data.whiteTime,
                  blackTime: data.blackTime,
                  status: data.status,
                  spectatorCount: data.spectatorCount,
                  white: data.white,
                  black: data.black,
                  result: data.result,
                });
                break;
              }
              case "move": {
                setGameState((previousState) => {
                  if (!previousState) {
                    return previousState;
                  }

                  let nextFen = data.fen ?? previousState.fen;

                  try {
                    chessRef.current.move({
                      from: data.from,
                      to: data.to,
                      promotion: data.promotion ?? "q",
                    });
                    if (!data.fen) {
                      nextFen = chessRef.current.fen();
                    }
                  } catch {
                    nextFen = previousState.fen;
                  }

                  return {
                    ...previousState,
                    fen: nextFen,
                    moves: [
                      ...previousState.moves,
                      {
                        from: data.from,
                        to: data.to,
                        san: data.san,
                        color: data.color,
                        promotion: data.promotion,
                      },
                    ],
                  };
                });
                break;
              }
              case "clock": {
                setGameState((previousState) =>
                  previousState
                    ? {
                        ...previousState,
                        whiteTime: data.whiteTime,
                        blackTime: data.blackTime,
                      }
                    : previousState,
                );
                break;
              }
              case "spectator_count": {
                setGameState((previousState) =>
                  previousState
                    ? {
                        ...previousState,
                        spectatorCount: data.count,
                      }
                    : previousState,
                );
                break;
              }
              case "game_over": {
                setGameState((previousState) =>
                  previousState
                    ? {
                        ...previousState,
                        status: normalizeGameOverReason(data.reason),
                        result: data.result,
                      }
                    : previousState,
                );
                break;
              }
            }
          } catch {
            setStatus("error");
          }
        };

        ws.onerror = () => {
          setStatus("error");
        };

        ws.onclose = () => {
          wsRef.current = null;

          if (isManualDisconnectRef.current) {
            isManualDisconnectRef.current = false;
            setStatus("idle");
            return;
          }

          setStatus("disconnected");

          if (!isOnlineRef.current || usingMockDataRef.current) {
            return;
          }

          if (reconnectAttemptsRef.current >= MAX_RECONNECT_ATTEMPTS) {
            setStatus("error");
            return;
          }

          const delay = calculateReconnectDelay(reconnectAttemptsRef.current);
          reconnectTimeoutRef.current = setTimeout(() => {
            reconnectAttemptsRef.current += 1;
            createWebSocket(true);
          }, delay);

          reconnectTimerRef.current = setTimeout(() => {
            if (status === "reconnecting") {
              setStatus("reconnecting");
            }
          }, RECONNECT_TIMEOUT);
        };

        return ws;
      } catch {
        setStatus("error");
        return null;
      }
    },
    [bootstrapMockState, calculateReconnectDelay, gameId, status],
  );

  const disconnect = useCallback(() => {
    isManualDisconnectRef.current = true;
    clearReconnectTimeout();
    reconnectAttemptsRef.current = 0;
    usingMockDataRef.current = false;

    if (wsRef.current) {
      wsRef.current.close(1000, "Manual disconnect");
      wsRef.current = null;
    } else {
      setStatus(gameId ? "idle" : "idle");
    }
  }, [clearReconnectTimeout, gameId]);

  const reconnect = useCallback(() => {
    if (!gameId) {
      setStatus("idle");
      return;
    }

    clearReconnectTimeout();
    reconnectAttemptsRef.current = 0;
    isManualDisconnectRef.current = false;
    usingMockDataRef.current = false;

    if (wsRef.current) {
      wsRef.current.close();
      wsRef.current = null;
    }

    setGameState(null);
    createWebSocket(true);
  }, [clearReconnectTimeout, createWebSocket, gameId]);

  useEffect(() => {
    const handleOnline = () => {
      isOnlineRef.current = true;
      if (status === "disconnected" && gameId && !usingMockDataRef.current) {
        reconnectAttemptsRef.current = 0;
        createWebSocket(true);
      }
    };

    const handleOffline = () => {
      isOnlineRef.current = false;
      clearReconnectTimeout();
    };

    window.addEventListener("online", handleOnline);
    window.addEventListener("offline", handleOffline);

    return () => {
      window.removeEventListener("online", handleOnline);
      window.removeEventListener("offline", handleOffline);
    };
  }, [clearReconnectTimeout, createWebSocket, gameId, status]);

  useEffect(() => {
    if (!gameId) {
      clearReconnectTimeout();
      setStatus("idle");
      setGameState(null);
      return;
    }

    const ws = createWebSocket(false);

    return () => {
      isManualDisconnectRef.current = true;
      clearReconnectTimeout();
      usingMockDataRef.current = false;
      if (ws) {
        ws.close();
      }
    };
  }, [clearReconnectTimeout, createWebSocket, gameId]);

  useEffect(() => {
    return () => {
      clearReconnectTimeout();
      if (wsRef.current) {
        wsRef.current.close();
      }
    };
  }, [clearReconnectTimeout]);

  return {
    status,
    gameState,
    disconnect,
    reconnect,
  };
}
