import { useEffect, useRef, useState, useCallback } from "react";
import { endpoints } from "@/lib/api";

export type ChessSocketStatus =
  | "idle"
  | "connecting"
  | "connected"
  | "reconnecting"
  | "disconnected"
  | "error";

export interface ChessMove {
  from: string;
  to: string;
  promotion?: string;
  /**
   * Half-move (ply) index this move produces. Stamp it on every outgoing move so
   * the server can reject duplicates and the client can reconcile its offline
   * queue against the server's authoritative state (FE-114).
   */
  ply?: number;
}

/**
 * Authoritative board snapshot returned by the server's `sync` reply. The hook
 * exposes it so the parent (e.g. `PlayGameEngine`) can restore the board after a
 * reconnect and apply the opponent's missed moves (FE-113).
 */
export interface SyncedGameState {
  fen: string;
  moves: ChessMove[];
  whiteTime?: number;
  blackTime?: number;
  status?: string;
  /** Highest ply the server has applied; used to drop already-sent moves. */
  lastPly: number;
}

interface UseChessSocketReturn {
  status: ChessSocketStatus;
  gameId: string | null;
  lastOpponentMove: ChessMove | null;
  /** True once the opponent has resigned (FE-117 / #1358). */
  opponentResigned: boolean;
  lastSyncedState: SyncedGameState | null;
  sendMove: (move: ChessMove) => void;
  /** Sends an explicit resign message to the server (FE-117 / #1358). */
  sendResign: () => void;
  disconnect: () => void;
  reconnect: () => void;
}

const MAX_RECONNECT_ATTEMPTS = 10;
const INITIAL_RECONNECT_DELAY = 1000; // 1 second
const MAX_RECONNECT_DELAY = 30000; // 30 seconds
const RECONNECT_TIMEOUT = 3000; // 3 seconds timeout for reconnection

function toMove(raw: unknown): ChessMove | null {
  if (!raw || typeof raw !== "object") return null;
  const r = raw as Record<string, unknown>;
  if (typeof r.from !== "string" || typeof r.to !== "string") return null;
  return {
    from: r.from,
    to: r.to,
    promotion: typeof r.promotion === "string" ? r.promotion : undefined,
    ply: typeof r.ply === "number" ? r.ply : undefined,
  };
}

/**
 * Custom hook for managing WebSocket connection to the chess game server.
 * Handles connection, reconnection with exponential backoff, move queuing, and network state detection.
 *
 * @param gameId - The unique identifier for the chess game, or null if no game is active
 * @returns Object containing WebSocket status, game state, and control functions
 */
export function useChessSocket(gameId: string | null): UseChessSocketReturn {
  const [status, setStatusState] = useState<ChessSocketStatus>("idle");
  // Wrapper so statusRef always mirrors the React state value.
  const setStatus = useCallback((s: ChessSocketStatus) => {
    statusRef.current = s;
    setStatusState(s);
  }, []);
  const [lastOpponentMove, setLastOpponentMove] = useState<ChessMove | null>(null);
  const [opponentResigned, setOpponentResigned] = useState(false);
  const [lastSyncedState, setLastSyncedState] = useState<SyncedGameState | null>(null);

  
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectAttemptsRef = useRef(0);
  const reconnectTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const reconnectTimerRef = useRef<NodeJS.Timeout | null>(null);
  const moveQueueRef = useRef<ChessMove[]>([]);
  const isManualDisconnectRef = useRef(false);
  const isOnlineRef = useRef(typeof navigator !== 'undefined' ? navigator.onLine : true);
  // Keep a ref to the latest status so callbacks can read it without being
  // re-created every time status changes (prevents infinite reconnect loop).
  const statusRef = useRef<ChessSocketStatus>("idle");
  // Highest ply the server has acknowledged/applied. Moves at or below this
  // index are known to have reached the server and must not be replayed (FE-114).
  const lastAckedPlyRef = useRef(0);
  // True while we are waiting for the server's `sync` reply after a reconnect.
  // The offline queue is only flushed once that authoritative state arrives.
  const awaitingSyncRef = useRef(false);

  /**
   * Clears all pending reconnection timeouts and timers.
   */
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

  /**
   * Calculates the delay for the next reconnection attempt using exponential backoff with jitter.
   */
  const calculateReconnectDelay = useCallback((attempt: number): number => {
    const baseDelay = INITIAL_RECONNECT_DELAY * Math.pow(2, attempt);
    const jitter = Math.random() * 0.3 * baseDelay; // Add 0-30% jitter
    const delay = baseDelay + jitter;
    return Math.min(delay, MAX_RECONNECT_DELAY);
  }, []);

  /**
   * Sends a move over the socket, always stamping the ply index so the server
   * can de-duplicate and the client can reconcile it later (FE-114).
   */
  const dispatchMove = useCallback(
    (ws: WebSocket, move: ChessMove) => {
      ws.send(
        JSON.stringify({
          type: "move",
          gameId,
          from: move.from,
          to: move.to,
          promotion: move.promotion,
          ply: move.ply,
        }),
      );
    },
    [gameId],
  );

  /**
   * Replays queued offline moves that the server has not applied yet. Moves at or
   * below `lastAckedPlyRef` are dropped because the server already has them.
   */
  const flushQueuedMoves = useCallback(
    (ws: WebSocket) => {
      const pending = moveQueueRef.current.filter(
        (move) => (move.ply ?? Number.POSITIVE_INFINITY) > lastAckedPlyRef.current,
      );
      const dropped = moveQueueRef.current.length - pending.length;
      if (dropped > 0) {
        console.log(`[WebSocket] Dropped ${dropped} queued move(s) already applied by the server`);
      }
      if (pending.length > 0) {
        console.log(`[WebSocket] Dispatching ${pending.length} queued move(s)`);
        pending.forEach((move) => dispatchMove(ws, move));
      }
      moveQueueRef.current = [];
    },
    [dispatchMove],
  );

  /**
   * Creates a new WebSocket connection to the game server configured via
   * `NEXT_PUBLIC_API_URL` (see `endpoints.games.ws`).
   *
   * @param attemptReconnect - Whether this connection attempt is a reconnection
   */
  const createWebSocket = useCallback(async (attemptReconnect = false) => {
    if (!gameId) return null;

    try {
      const wsUrl = endpoints.games.ws(gameId);

      const ws = new WebSocket(wsUrl);
      wsRef.current = ws;

      if (attemptReconnect) {
        setStatus("reconnecting");
        console.log(`[WebSocket] Attempting reconnection to ${wsUrl}`);
      } else {
        setStatus("connecting");
        console.log(`[WebSocket] Connecting to ${wsUrl} for game ${gameId}`);
      }

      ws.onopen = () => {
        console.log(`[WebSocket] ${attemptReconnect ? 'Reconnected' : 'Connected'} for game ${gameId}`);
        setStatus("connected");
        reconnectAttemptsRef.current = 0;

        if (attemptReconnect) {
          // Ask for the authoritative board first; the offline queue is flushed
          // in the `sync` handler once we know which moves the server applied.
          awaitingSyncRef.current = true;
          console.log(`[WebSocket] Requesting board state sync for game ${gameId}`);
          ws.send(JSON.stringify({ type: "sync", gameId }));
        } else {
          // Fresh connection: any moves queued before it opened can be sent now.
          flushQueuedMoves(ws);
        }
      };

      ws.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data);
          if (data.type === "move") {
            console.log(`[WebSocket] Received opponent move:`, data);
            const move = toMove(data);
            if (move) {
              if (typeof move.ply === "number") {
                lastAckedPlyRef.current = Math.max(lastAckedPlyRef.current, move.ply);
              }
              setLastOpponentMove(move);
            }
          } else if (data.type === "ack") {
            // The server confirms it applied a move; drop it from the queue.
            if (typeof data.ply === "number") {
              lastAckedPlyRef.current = Math.max(lastAckedPlyRef.current, data.ply);
              moveQueueRef.current = moveQueueRef.current.filter(
                (move) => (move.ply ?? 0) > lastAckedPlyRef.current,
              );
            }
          } else if (data.type === "resign" || (data.type === "game_over" && /resign/i.test(String(data.reason ?? "")))) {
            console.log(`[WebSocket] Opponent resigned`);
            setOpponentResigned(true);
          } else if (data.type === "sync") {
            console.log(`[WebSocket] Received board state sync:`, data);
            const lastPly =
              typeof data.lastPly === "number"
                ? data.lastPly
                : typeof data.ply === "number"
                  ? data.ply
                  : lastAckedPlyRef.current;
            lastAckedPlyRef.current = Math.max(lastAckedPlyRef.current, lastPly);
            const moves: ChessMove[] = Array.isArray(data.moves)
              ? (data.moves.map(toMove).filter(Boolean) as ChessMove[])
              : [];
            setLastSyncedState({
              fen: typeof data.fen === "string" ? data.fen : "",
              moves,
              whiteTime: typeof data.whiteTime === "number" ? data.whiteTime : undefined,
              blackTime: typeof data.blackTime === "number" ? data.blackTime : undefined,
              status: typeof data.status === "string" ? data.status : undefined,
              lastPly,
            });
            // Now that we know the server's position, reconcile and flush.
            if (awaitingSyncRef.current) {
              awaitingSyncRef.current = false;
              if (wsRef.current?.readyState === WebSocket.OPEN) {
                flushQueuedMoves(wsRef.current);
              }
            }
          }
        } catch (error) {
          console.error("[WebSocket] Failed to parse message:", error);
        }
      };

      ws.onerror = (error) => {
        console.error("[WebSocket] Error:", error);
        setStatus("error");
      };

      ws.onclose = (event) => {
        console.log(`[WebSocket] Closed for game ${gameId}. Code: ${event.code}, Reason: ${event.reason}`);
        
        if (!isManualDisconnectRef.current) {
          setStatus("disconnected");
          
          // Check if we're online before attempting reconnection
          if (!isOnlineRef.current) {
            console.log("[WebSocket] Device is offline, waiting for network...");
            return;
          }
          
          // Inline reconnection logic to avoid circular dependency
          if (reconnectAttemptsRef.current < MAX_RECONNECT_ATTEMPTS) {
            const delay = calculateReconnectDelay(reconnectAttemptsRef.current);
            console.log(`[WebSocket] Attempting reconnection ${reconnectAttemptsRef.current + 1}/${MAX_RECONNECT_ATTEMPTS} in ${Math.round(delay)}ms`);
            
            reconnectTimeoutRef.current = setTimeout(() => {
              reconnectAttemptsRef.current++;
              createWebSocket(true);
            }, delay);

            // Set a timeout to ensure reconnection completes within 3 seconds
            reconnectTimerRef.current = setTimeout(() => {
              if (statusRef.current === "reconnecting") {
                console.log("[WebSocket] Reconnection timeout exceeded 3 seconds");
                // Continue trying but log the timeout
              }
            }, RECONNECT_TIMEOUT);
          } else {
            console.log("[WebSocket] Max reconnection attempts reached");
            setStatus("error");
          }
        } else {
          setStatus("idle");
          isManualDisconnectRef.current = false;
        }
      };

      return ws;
    } catch (error) {
      console.error("[WebSocket] Failed to create:", error);
      setStatus("error");
      return null;
    }
  }, [gameId, calculateReconnectDelay, flushQueuedMoves]);

  /**
   * Sends a chess move to the server.
   * If the WebSocket is connected, the move is sent immediately.
   * If disconnected, the move is queued and will be sent upon reconnection.
   * 
   * @param move - The chess move to send, containing source and target squares
   */
  const sendMove = useCallback((move: ChessMove) => {
    // If WebSocket is open, send immediately
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      dispatchMove(wsRef.current, move);
      console.log("[WebSocket] Move sent immediately:", move);
    } else {
      // Queue the move for when we reconnect
      moveQueueRef.current.push(move);
      console.log("[WebSocket] Move queued (disconnected):", move);
      
      // Start reconnection if not already attempting
      if (statusRef.current === "disconnected" || statusRef.current === "idle") {
        reconnectAttemptsRef.current = 0;
        createWebSocket(true);
      }
    }
  }, [createWebSocket, dispatchMove]);

  /**
   * Sends an explicit resign message so the server can complete the game and
   * notify the opponent, instead of just dropping the socket (FE-117 / #1358).
   */
  const sendResign = useCallback(() => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify({ type: "resign", gameId }));
      console.log("[WebSocket] Resign message sent");
    } else {
      console.warn("[WebSocket] Cannot resign — socket is not open");
    }
  }, [gameId]);

  /**
   * Manually disconnects from the WebSocket server.
   * Clears the move queue and prevents automatic reconnection.
   */
  const disconnect = useCallback(() => {
    isManualDisconnectRef.current = true;
    clearReconnectTimeout();
    
    if (wsRef.current) {
      wsRef.current.close(1000, "Manual disconnect");
      wsRef.current = null;
    }
    
    // Clear move queue on manual disconnect
    moveQueueRef.current = [];
    reconnectAttemptsRef.current = 0;
    console.log("[WebSocket] Manually disconnected");
  }, [clearReconnectTimeout]);

  /**
   * Manually triggers a reconnection attempt.
   * Closes the existing connection and initiates a new one.
   */
  const reconnect = useCallback(() => {
    isManualDisconnectRef.current = false;
    clearReconnectTimeout();
    
    if (wsRef.current) {
      wsRef.current.close();
      wsRef.current = null;
    }
    
    reconnectAttemptsRef.current = 0;
    console.log("[WebSocket] Manual reconnection initiated");
    createWebSocket(true);
  }, [clearReconnectTimeout, createWebSocket]);

  // Handle online/offline events for network state detection
  useEffect(() => {
    /**
     * Handles the browser online event.
     * Triggers reconnection when the device comes back online.
     */
    const handleOnline = () => {
      console.log("[Network] Device is online");
      isOnlineRef.current = true;
      
      // If we were disconnected and are now online, attempt reconnection
      if (statusRef.current === "disconnected" && gameId) {
        console.log("[Network] Online detected, attempting reconnection...");
        reconnectAttemptsRef.current = 0;
        createWebSocket(true);
      }
    };

    /**
     * Handles the browser offline event.
     * Clears pending reconnection attempts when the device goes offline.
     */
    const handleOffline = () => {
      console.log("[Network] Device is offline");
      isOnlineRef.current = false;
      
      // Clear any pending reconnection attempts
      clearReconnectTimeout();
    };

    if (typeof window !== 'undefined') {
      window.addEventListener('online', handleOnline);
      window.addEventListener('offline', handleOffline);
    }

    return () => {
      if (typeof window !== 'undefined') {
        window.removeEventListener('online', handleOnline);
        window.removeEventListener('offline', handleOffline);
      }
    };
  }, [gameId, createWebSocket, clearReconnectTimeout]);

  // Initialize WebSocket when gameId changes
  useEffect(() => {
    let active = true;
    setOpponentResigned(false);
    if (gameId) {
      createWebSocket().then(ws => {
        if (!active && ws) {
          ws.close();
        }
      });

      return () => {
        active = false;
        isManualDisconnectRef.current = true;
        clearReconnectTimeout();
        if (wsRef.current) {
          wsRef.current.close();
          wsRef.current = null;
        }
      };
    } else {
      setStatus("idle");
      setLastOpponentMove(null);
      setLastSyncedState(null);
    }
  }, [gameId, createWebSocket, clearReconnectTimeout]);

  // Cleanup on unmount
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
    gameId,
    lastOpponentMove,
    opponentResigned,
    lastSyncedState,
    sendMove,
    sendResign,
    disconnect,
    reconnect,
  };
}
