import { useEffect, useRef, useState, useCallback } from "react";
import { endpoints, requestWebSocketTicket } from "@/lib/api";
import { debugLog } from "@/lib/debugLogger";

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

/**
 * In-game negotiation actions (BE-170): draw, abort, takeback and rematch.
 * Outgoing frames are `{ type, gameId, ...extra }`; the server relays the same
 * `type` to the opponent and reports final outcomes via `game_over`.
 */
export type GameActionType =
  | "draw_offer"
  | "draw_accept"
  | "draw_decline"
  | "abort"
  | "takeback_offer"
  | "takeback_accept"
  | "takeback_decline"
  | "rematch_offer"
  | "rematch_accept"
  | "rematch_decline";

/** An action/event received from the server (relayed from the opponent). */
export interface GameActionEvent {
  /** Monotonic id so identical consecutive events still trigger effects. */
  id: number;
  type: GameActionType | "game_over";
  /** For `takeback_accept`: ply count the board should be rolled back to. */
  ply?: number;
  /** For `rematch_accept`: id of the newly created game. */
  newGameId?: string;
  /** For `game_over`: reason string (e.g. "draw_agreed", "aborted"). */
  reason?: string;
}

/**
 * Authoritative clock snapshot from the server (BE-149). Times are in ms and
 * describe the clocks at `serverTime`; `receivedAt` is the local
 * `performance.now()` when the frame arrived.
 */
export interface ServerClockState {
  whiteMs: number;
  blackMs: number;
  /** Side whose clock is running, or null when paused/finished. */
  turn: "w" | "b" | null;
  /** Server wall-clock (epoch ms) at which the snapshot was taken, if sent. */
  serverTime?: number;
  receivedAt: number;
}

interface UseChessSocketReturn {
  status: ChessSocketStatus;
  gameId: string | null;
  lastOpponentMove: ChessMove | null;
  /** True once the opponent has resigned (FE-117 / #1358). */
  opponentResigned: boolean;
  lastSyncedState: SyncedGameState | null;
  /** Latest negotiation event from the opponent/server (BE-170). */
  lastGameAction: GameActionEvent | null;
  /** Latest authoritative clock snapshot from the server (BE-149). */
  serverClock: ServerClockState | null;
  /** Smoothed round-trip time in ms, measured via ping/pong. */
  latencyMs: number;
  /** Estimated `serverNow - Date.now()` in ms (0 until measured). */
  clockOffsetMs: number;
  sendMove: (move: ChessMove) => void;
  /** Sends an explicit resign message to the server (FE-117 / #1358). */
  sendResign: () => void;
  /** Sends a draw/abort/takeback/rematch action (BE-170). Returns false if offline. */
  sendGameAction: (type: GameActionType, extra?: Record<string, unknown>) => boolean;
  disconnect: () => void;
  reconnect: () => void;
}

const PING_INTERVAL_MS = 5000;

/** Parses clock fields from any server frame (ms preferred, seconds accepted). */
export function parseClock(
  data: Record<string, unknown>,
  receivedAt: number,
): ServerClockState | null {
  const num = (v: unknown) => (typeof v === "number" && Number.isFinite(v) ? v : undefined);
  const clock =
    data.clock && typeof data.clock === "object"
      ? (data.clock as Record<string, unknown>)
      : data;
  let whiteMs = num(clock.whiteMs) ?? num(clock.white_ms);
  let blackMs = num(clock.blackMs) ?? num(clock.black_ms);
  if (whiteMs === undefined || blackMs === undefined) {
    // Only treat seconds fields as a clock frame when this is an explicit clock message.
    if (data.type !== "clock") return null;
    const ws = num(clock.whiteTime);
    const bs = num(clock.blackTime);
    if (ws === undefined || bs === undefined) return null;
    whiteMs = ws * 1000;
    blackMs = bs * 1000;
  }
  const rawTurn = clock.turn ?? clock.activeColor ?? clock.active_color;
  const turn =
    rawTurn === "w" || rawTurn === "white"
      ? "w"
      : rawTurn === "b" || rawTurn === "black"
        ? "b"
        : null;
  return {
    whiteMs,
    blackMs,
    turn,
    serverTime: num(clock.serverTime) ?? num(clock.server_time) ?? num(data.serverTime),
    receivedAt,
  };
}

/**
 * Remaining ms for each side "now", correcting a server snapshot for transit
 * latency. With a server timestamp we use the measured clock offset; otherwise
 * we assume the frame spent half the RTT in flight.
 */
export function projectClock(
  clock: ServerClockState,
  nowPerf: number,
  nowEpoch: number,
  latencyMs: number,
  clockOffsetMs: number,
): { whiteMs: number; blackMs: number } {
  let elapsed: number;
  if (typeof clock.serverTime === "number" && clockOffsetMs !== 0) {
    elapsed = nowEpoch + clockOffsetMs - clock.serverTime;
  } else {
    elapsed = nowPerf - clock.receivedAt + latencyMs / 2;
  }
  elapsed = Math.max(0, elapsed);
  return {
    whiteMs: Math.max(0, clock.whiteMs - (clock.turn === "w" ? elapsed : 0)),
    blackMs: Math.max(0, clock.blackMs - (clock.turn === "b" ? elapsed : 0)),
  };
}

const GAME_ACTION_TYPES = new Set<string>([
  "draw_offer",
  "draw_accept",
  "draw_decline",
  "abort",
  "takeback_offer",
  "takeback_accept",
  "takeback_decline",
  "rematch_offer",
  "rematch_accept",
  "rematch_decline",
]);

/** Maps legacy PascalCase server names (backend/src/socket) onto the action set. */
const LEGACY_ACTION_ALIASES: Record<string, GameActionType> = {
  TakebackOffered: "takeback_offer",
  TakebackAccepted: "takeback_accept",
  TakebackRejected: "takeback_decline",
};

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
  const [lastGameAction, setLastGameAction] = useState<GameActionEvent | null>(null);
  const [serverClock, setServerClock] = useState<ServerClockState | null>(null);
  const [latencyMs, setLatencyMs] = useState(0);
  const [clockOffsetMs, setClockOffsetMs] = useState(0);
  const actionIdRef = useRef(0);
  const latencyRef = useRef(0);
  const pingTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const wsRef = useRef<WebSocket | null>(null);
  const createWebSocketRef = useRef<((attemptReconnect: boolean) => Promise<WebSocket | null>) | null>(null);
  const connectionRequestIdRef = useRef(0);
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
        debugLog(`[WebSocket] Dropped ${dropped} queued move(s) already applied by the server`);
      }
      if (pending.length > 0) {
        debugLog(`[WebSocket] Dispatching ${pending.length} queued move(s)`);
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

    const requestId = ++connectionRequestIdRef.current;
    setStatus(attemptReconnect ? "reconnecting" : "connecting");
    try {
      const ticket = await requestWebSocketTicket();
      if (requestId !== connectionRequestIdRef.current || isManualDisconnectRef.current) {
        return null;
      }
      const baseSocketUrl = endpoints.games.ws(gameId);
      const socketUrl = new URL(baseSocketUrl);
      socketUrl.searchParams.set("ticket", ticket);
      const wsUrl = socketUrl.toString();

      const ws = new WebSocket(wsUrl);
      wsRef.current = ws;

      if (attemptReconnect) {
        setStatus("reconnecting");
        debugLog(`[WebSocket] Attempting reconnection to ${baseSocketUrl}`);
      } else {
        setStatus("connecting");
        debugLog(`[WebSocket] Connecting to ${baseSocketUrl} for game ${gameId}`);
      }

      ws.onopen = () => {
        debugLog(`[WebSocket] ${attemptReconnect ? 'Reconnected' : 'Connected'} for game ${gameId}`);
        setStatus("connected");
        reconnectAttemptsRef.current = 0;

        // Latency probe (BE-149): RTT drives clock latency correction.
        const sendPing = () => {
          if (ws.readyState === WebSocket.OPEN) {
            ws.send(JSON.stringify({ type: "ping", gameId, clientTime: Date.now() }));
          }
        };
        if (pingTimerRef.current) clearInterval(pingTimerRef.current);
        sendPing();
        pingTimerRef.current = setInterval(sendPing, PING_INTERVAL_MS);

        if (attemptReconnect) {
          // Ask for the authoritative board first; the offline queue is flushed
          // in the `sync` handler once we know which moves the server applied.
          awaitingSyncRef.current = true;
          debugLog(`[WebSocket] Requesting board state sync for game ${gameId}`);
          ws.send(JSON.stringify({ type: "sync", gameId }));
        } else {
          // Fresh connection: any moves queued before it opened can be sent now.
          flushQueuedMoves(ws);
        }
      };

      ws.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data);
          const receivedAt = typeof performance !== "undefined" ? performance.now() : Date.now();

          // Any frame may carry an authoritative clock snapshot (BE-149).
          if (data && typeof data === "object") {
            const clock = parseClock(data as Record<string, unknown>, receivedAt);
            if (clock) setServerClock(clock);
          }

          const actionType: string | undefined =
            LEGACY_ACTION_ALIASES[data.type as string] ??
            (GAME_ACTION_TYPES.has(data.type) ? data.type : undefined);

          if (data.type === "pong") {
            if (typeof data.clientTime === "number") {
              const now = Date.now();
              const rtt = Math.max(0, now - data.clientTime);
              // Exponential smoothing keeps one slow frame from jerking the clock.
              const smoothed =
                latencyRef.current === 0 ? rtt : latencyRef.current * 0.7 + rtt * 0.3;
              latencyRef.current = smoothed;
              setLatencyMs(Math.round(smoothed));
              if (typeof data.serverTime === "number") {
                setClockOffsetMs(Math.round(data.serverTime - (data.clientTime + rtt / 2)));
              }
            }
          } else if (data.type === "clock") {
            // Already handled by parseClock above.
          } else if (actionType) {
            setLastGameAction({
              id: ++actionIdRef.current,
              type: actionType as GameActionType,
              ply: typeof data.ply === "number" ? data.ply : undefined,
              newGameId:
                typeof data.newGameId === "string"
                  ? data.newGameId
                  : typeof data.new_game_id === "string"
                    ? data.new_game_id
                    : undefined,
            });
          } else if (
            data.type === "game_over" &&
            /draw|abort/i.test(String(data.reason ?? ""))
          ) {
            setLastGameAction({
              id: ++actionIdRef.current,
              type: "game_over",
              reason: String(data.reason),
            });
          } else if (data.type === "move") {
            debugLog(`[WebSocket] Received opponent move:`, data);
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
            debugLog(`[WebSocket] Opponent resigned`);
            setOpponentResigned(true);
          } else if (data.type === "sync") {
            debugLog(`[WebSocket] Received board state sync:`, data);
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
        debugLog(`[WebSocket] Closed for game ${gameId}. Code: ${event.code}, Reason: ${event.reason}`);
        if (pingTimerRef.current) {
          clearInterval(pingTimerRef.current);
          pingTimerRef.current = null;
        }

        if (!isManualDisconnectRef.current) {
          setStatus("disconnected");
          
          // Check if we're online before attempting reconnection
          if (!isOnlineRef.current) {
            debugLog("[WebSocket] Device is offline, waiting for network...");
            return;
          }
          
          // Inline reconnection logic to avoid circular dependency
          if (reconnectAttemptsRef.current < MAX_RECONNECT_ATTEMPTS) {
            const delay = calculateReconnectDelay(reconnectAttemptsRef.current);
            debugLog(`[WebSocket] Attempting reconnection ${reconnectAttemptsRef.current + 1}/${MAX_RECONNECT_ATTEMPTS} in ${Math.round(delay)}ms`);
            
            reconnectTimeoutRef.current = setTimeout(() => {
              reconnectAttemptsRef.current++;
              createWebSocket(true);
            }, delay);

            // Set a timeout to ensure reconnection completes within 3 seconds
            reconnectTimerRef.current = setTimeout(() => {
              if (statusRef.current === "reconnecting") {
                debugLog("[WebSocket] Reconnection timeout exceeded 3 seconds");
                // Continue trying but log the timeout
              }
            }, RECONNECT_TIMEOUT);
          } else {
            debugLog("[WebSocket] Max reconnection attempts reached");
            setStatus("error");
          }
        } else {
          setStatus("idle");
          isManualDisconnectRef.current = false;
        }
      };

      return ws;
    } catch (error) {
      if (requestId !== connectionRequestIdRef.current || isManualDisconnectRef.current) {
        return null;
      }
      console.error("[WebSocket] Failed to obtain ticket or create socket:", error);
      setStatus("error");
      if (!isManualDisconnectRef.current && reconnectAttemptsRef.current < MAX_RECONNECT_ATTEMPTS) {
        const delay = calculateReconnectDelay(reconnectAttemptsRef.current);
        reconnectTimeoutRef.current = setTimeout(() => {
          reconnectAttemptsRef.current++;
          void createWebSocketRef.current?.(true);
        }, delay);
      }
      return null;
    }
  }, [gameId, calculateReconnectDelay, flushQueuedMoves]);
  createWebSocketRef.current = createWebSocket;

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
      debugLog("[WebSocket] Move sent immediately:", move);
    } else {
      // Queue the move for when we reconnect
      moveQueueRef.current.push(move);
      debugLog("[WebSocket] Move queued (disconnected):", move);
      
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
      debugLog("[WebSocket] Resign message sent");
    } else {
      console.warn("[WebSocket] Cannot resign — socket is not open");
    }
  }, [gameId]);

  /** Sends a draw/abort/takeback/rematch frame (BE-170). */
  const sendGameAction = useCallback(
    (type: GameActionType, extra: Record<string, unknown> = {}) => {
      if (wsRef.current?.readyState === WebSocket.OPEN) {
        wsRef.current.send(JSON.stringify({ ...extra, type, gameId }));
        debugLog(`[WebSocket] Game action sent: ${type}`);
        return true;
      }
      console.warn(`[WebSocket] Cannot send ${type} — socket is not open`);
      return false;
    },
    [gameId],
  );

  /**
   * Manually disconnects from the WebSocket server.
   * Clears the move queue and prevents automatic reconnection.
   */
  const disconnect = useCallback(() => {
    isManualDisconnectRef.current = true;
    connectionRequestIdRef.current++;
    clearReconnectTimeout();
    
    if (wsRef.current) {
      wsRef.current.close(1000, "Manual disconnect");
      wsRef.current = null;
    }
    
    // Clear move queue on manual disconnect
    moveQueueRef.current = [];
    reconnectAttemptsRef.current = 0;
    debugLog("[WebSocket] Manually disconnected");
  }, [clearReconnectTimeout]);

  /**
   * Manually triggers a reconnection attempt.
   * Closes the existing connection and initiates a new one.
   */
  const reconnect = useCallback(() => {
    isManualDisconnectRef.current = true;
    clearReconnectTimeout();
    
    if (wsRef.current) {
      wsRef.current.close();
      wsRef.current = null;
    }

    reconnectAttemptsRef.current = 0;
    isManualDisconnectRef.current = false;
    debugLog("[WebSocket] Manual reconnection initiated");
    createWebSocket(true);
  }, [clearReconnectTimeout, createWebSocket]);

  // Handle online/offline events for network state detection
  useEffect(() => {
    /**
     * Handles the browser online event.
     * Triggers reconnection when the device comes back online.
     */
    const handleOnline = () => {
      debugLog("[Network] Device is online");
      isOnlineRef.current = true;
      
      // If we were disconnected and are now online, attempt reconnection
      if (statusRef.current === "disconnected" && gameId) {
        debugLog("[Network] Online detected, attempting reconnection...");
        reconnectAttemptsRef.current = 0;
        createWebSocket(true);
      }
    };

    /**
     * Handles the browser offline event.
     * Clears pending reconnection attempts when the device goes offline.
     */
    const handleOffline = () => {
      debugLog("[Network] Device is offline");
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
    setLastGameAction(null);
    setServerClock(null);
    if (gameId) {
      isManualDisconnectRef.current = false;
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
      if (pingTimerRef.current) clearInterval(pingTimerRef.current);
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
    lastGameAction,
    serverClock,
    latencyMs,
    clockOffsetMs,
    sendMove,
    sendResign,
    sendGameAction,
    disconnect,
    reconnect,
  };
}
