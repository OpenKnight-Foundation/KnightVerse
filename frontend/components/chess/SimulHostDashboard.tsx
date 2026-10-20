"use client";

import React, {
  useCallback,
  useEffect,
  useReducer,
  useMemo,
} from "react";
import { FaChessKnight, FaClock, FaTrophy, FaExclamationTriangle } from "react-icons/fa";

// ── Types ─────────────────────────────────────────────────────────────────────

export type SimulGameStatus =
  | "waiting"
  | "active"
  | "host_turn"
  | "opponent_turn"
  | "checkmate_host"
  | "checkmate_opponent"
  | "draw"
  | "resigned";

export interface SimulGame {
  id: string;
  opponentName: string;
  opponentElo: number;
  /** Current FEN for this board. */
  fen: string;
  status: SimulGameStatus;
  /** Remaining time in seconds for the host on this board. */
  hostTimeRemaining: number;
  /** Remaining time in seconds for the opponent. */
  opponentTimeRemaining: number;
  /** Number of moves played. */
  moveCount: number;
  /** Last move in SAN notation. */
  lastMove?: string;
  /** Board index (0-based) assigned for display ordering. */
  boardIndex: number;
}

export interface SimulState {
  games: SimulGame[];
  /** Index of the currently focused board (null = none). */
  focusedBoardId: string | null;
  /** Whether the host is in spectator view (read-only). */
  isSpectatorMode: boolean;
  hostName: string;
}

// ── Actions ───────────────────────────────────────────────────────────────────

type SimulAction =
  | { type: "FOCUS_BOARD"; id: string }
  | { type: "FOCUS_NEXT_ACTIVE" }
  | { type: "FOCUS_PREV_ACTIVE" }
  | { type: "MAKE_MOVE"; gameId: string; san: string; newFen: string }
  | { type: "UPDATE_GAME"; game: Partial<SimulGame> & { id: string } }
  | { type: "TICK" }
  | { type: "TOGGLE_SPECTATOR" };

// ── Reducer ───────────────────────────────────────────────────────────────────

function simulReducer(state: SimulState, action: SimulAction): SimulState {
  switch (action.type) {
    case "FOCUS_BOARD":
      return { ...state, focusedBoardId: action.id };

    case "FOCUS_NEXT_ACTIVE": {
      const active = state.games.filter((g) => g.status === "host_turn");
      if (!active.length) return state;
      const idx = active.findIndex((g) => g.id === state.focusedBoardId);
      const next = active[(idx + 1) % active.length];
      return { ...state, focusedBoardId: next?.id ?? state.focusedBoardId };
    }

    case "FOCUS_PREV_ACTIVE": {
      const active = state.games.filter((g) => g.status === "host_turn");
      if (!active.length) return state;
      const idx = active.findIndex((g) => g.id === state.focusedBoardId);
      const prev = active[(idx - 1 + active.length) % active.length];
      return { ...state, focusedBoardId: prev?.id ?? state.focusedBoardId };
    }

    case "MAKE_MOVE": {
      const games = state.games.map((g) =>
        g.id === action.gameId
          ? {
              ...g,
              fen: action.newFen,
              lastMove: action.san,
              moveCount: g.moveCount + 1,
              status: "opponent_turn" as SimulGameStatus,
            }
          : g
      );
      // Auto-advance focus to next host-turn board
      const nextActive = games.find(
        (g) => g.status === "host_turn" && g.id !== action.gameId
      );
      return {
        ...state,
        games,
        focusedBoardId: nextActive?.id ?? state.focusedBoardId,
      };
    }

    case "UPDATE_GAME": {
      const games = state.games.map((g) =>
        g.id === action.game.id ? { ...g, ...action.game } : g
      );
      return { ...state, games };
    }

    case "TICK": {
      const games = state.games.map((g) => {
        if (g.status === "host_turn" && g.hostTimeRemaining > 0) {
          return { ...g, hostTimeRemaining: g.hostTimeRemaining - 1 };
        }
        if (g.status === "opponent_turn" && g.opponentTimeRemaining > 0) {
          return { ...g, opponentTimeRemaining: g.opponentTimeRemaining - 1 };
        }
        return g;
      });
      return { ...state, games };
    }

    case "TOGGLE_SPECTATOR":
      return { ...state, isSpectatorMode: !state.isSpectatorMode };

    default:
      return state;
  }
}

// ── Constants ─────────────────────────────────────────────────────────────────

const MAX_BOARDS = 20;
const MIN_BOARDS = 2;

const STATUS_RING: Record<SimulGameStatus, string> = {
  waiting:            "ring-gray-700",
  active:             "ring-blue-500/60",
  host_turn:          "ring-yellow-500 ring-2 shadow-yellow-500/30 shadow-lg",
  opponent_turn:      "ring-gray-600",
  checkmate_host:     "ring-teal-500",
  checkmate_opponent: "ring-red-500",
  draw:               "ring-blue-400",
  resigned:           "ring-gray-700 opacity-60",
};

const STATUS_LABEL: Record<SimulGameStatus, string> = {
  waiting:            "Waiting",
  active:             "Active",
  host_turn:          "Your Turn",
  opponent_turn:      "Their Turn",
  checkmate_host:     "You Won ♛",
  checkmate_opponent: "You Lost",
  draw:               "Draw",
  resigned:           "Resigned",
};

// ── Helpers ───────────────────────────────────────────────────────────────────

function formatTime(seconds: number): string {
  if (seconds <= 0) return "0:00";
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

function urgencyClass(seconds: number): string {
  if (seconds <= 10) return "text-red-400 animate-pulse font-bold";
  if (seconds <= 30) return "text-orange-400 font-semibold";
  return "text-gray-300";
}

function isFinished(status: SimulGameStatus): boolean {
  return ["checkmate_host", "checkmate_opponent", "draw", "resigned"].includes(status);
}

// ── Mini board tile ───────────────────────────────────────────────────────────

interface MiniTileProps {
  game: SimulGame;
  isFocused: boolean;
  isSpectator: boolean;
  onFocus: (id: string) => void;
}

function MiniTile({ game, isFocused, isSpectator, onFocus }: MiniTileProps) {
  const finished = isFinished(game.status);
  const isHostTurn = game.status === "host_turn";

  return (
    <button
      type="button"
      onClick={() => onFocus(game.id)}
      aria-label={`Board ${game.boardIndex + 1}: ${game.opponentName}, ${STATUS_LABEL[game.status]}`}
      aria-pressed={isFocused}
      className={`relative flex flex-col rounded-xl border p-2.5 text-left transition-all duration-200 ring-1
        ${isFocused ? "border-indigo-500 bg-indigo-950/30" : "border-gray-700/60 bg-gray-800/50 hover:border-gray-600"}
        ${STATUS_RING[game.status]}
        ${finished ? "opacity-60" : ""}
      `}
    >
      {/* Board number badge */}
      <span className="absolute left-1.5 top-1.5 text-[10px] font-mono text-gray-500">
        #{game.boardIndex + 1}
      </span>

      {/* Status indicator */}
      {isHostTurn && !finished && (
        <span className="absolute right-1.5 top-1.5 h-2 w-2 rounded-full bg-yellow-400 animate-ping" />
      )}

      {/* Mini chessboard placeholder — 64 squares */}
      <div
        className="mx-auto mb-1.5 mt-3 grid grid-cols-8 rounded overflow-hidden"
        style={{ width: 64, height: 64 }}
        aria-hidden="true"
      >
        {Array.from({ length: 64 }, (_, i) => {
          const row = Math.floor(i / 8);
          const col = i % 8;
          const isLight = (row + col) % 2 === 0;
          return (
            <div
              key={i}
              className={isLight ? "bg-amber-100" : "bg-amber-800"}
              style={{ width: 8, height: 8 }}
            />
          );
        })}
      </div>

      {/* Opponent */}
      <p className="truncate text-[10px] font-semibold text-white leading-tight">
        {game.opponentName}
      </p>
      <p className="text-[9px] text-gray-500">{game.opponentElo} ELO</p>

      {/* Status */}
      <p
        className={`mt-1 text-[9px] font-semibold uppercase tracking-wide
          ${isHostTurn ? "text-yellow-400" : finished ? "text-gray-500" : "text-gray-400"}`}
      >
        {STATUS_LABEL[game.status]}
      </p>

      {/* Time */}
      {!finished && (
        <p className={`text-[9px] ${urgencyClass(game.hostTimeRemaining)}`}>
          <FaClock className="mr-0.5 inline text-[8px]" />
          {formatTime(game.hostTimeRemaining)}
        </p>
      )}

      {/* Spectator badge */}
      {isSpectator && (
        <span className="mt-1 rounded bg-blue-500/20 px-1 py-0.5 text-[8px] text-blue-400">
          Spectating
        </span>
      )}
    </button>
  );
}

// ── Focused board panel ───────────────────────────────────────────────────────

interface FocusedBoardProps {
  game: SimulGame;
  isSpectator: boolean;
  onMove?: (gameId: string, san: string, newFen: string) => void;
}

function FocusedBoard({ game, isSpectator }: FocusedBoardProps) {
  const finished = isFinished(game.status);
  const isHostTurn = game.status === "host_turn";

  return (
    <div className="flex flex-col gap-3 rounded-2xl border border-gray-700 bg-gray-900/80 p-4">
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-sm font-bold text-white">
            Board #{game.boardIndex + 1} — {game.opponentName}
          </h3>
          <p className="text-xs text-gray-400">{game.opponentElo} ELO · {game.moveCount} moves</p>
        </div>
        <span
          className={`rounded-full px-2 py-1 text-xs font-semibold
            ${isHostTurn ? "bg-yellow-500/20 text-yellow-300 animate-pulse" : "bg-gray-700 text-gray-400"}`}
        >
          {STATUS_LABEL[game.status]}
        </span>
      </div>

      {/* Opponent timer */}
      <div className="flex items-center justify-between rounded-lg bg-gray-800/60 px-3 py-2">
        <span className="text-xs text-gray-400">{game.opponentName}</span>
        <span className={`text-sm font-mono font-bold ${urgencyClass(game.opponentTimeRemaining)}`}>
          {formatTime(game.opponentTimeRemaining)}
        </span>
      </div>

      {/* Chessboard area */}
      <div
        className="mx-auto grid grid-cols-8 rounded-lg overflow-hidden border border-gray-700"
        style={{ width: 280, height: 280 }}
        aria-label={`Chessboard for game against ${game.opponentName}`}
        data-fen={game.fen}
      >
        {Array.from({ length: 64 }, (_, i) => {
          const row = Math.floor(i / 8);
          const col = i % 8;
          const isLight = (row + col) % 2 === 0;
          return (
            <div
              key={i}
              className={`${isLight ? "bg-amber-100" : "bg-amber-800"} hover:opacity-90 transition-opacity`}
              style={{ width: 35, height: 35 }}
            />
          );
        })}
      </div>

      {/* Host timer */}
      <div
        className={`flex items-center justify-between rounded-lg px-3 py-2
          ${isHostTurn && !finished ? "bg-yellow-900/30 border border-yellow-700/40" : "bg-gray-800/60"}`}
      >
        <span className="text-xs text-gray-400">You (Host)</span>
        <span className={`text-sm font-mono font-bold ${urgencyClass(game.hostTimeRemaining)}`}>
          {formatTime(game.hostTimeRemaining)}
        </span>
      </div>

      {/* Last move */}
      {game.lastMove && (
        <p className="text-center text-xs text-gray-500">
          Last: <span className="font-mono text-gray-300">{game.lastMove}</span>
        </p>
      )}

      {/* Spectator notice */}
      {isSpectator && (
        <p className="text-center text-xs text-blue-400">
          Spectator mode — boards are read-only
        </p>
      )}

      {/* No interaction prompt if host turn */}
      {isHostTurn && !isSpectator && !finished && (
        <p className="text-center text-xs text-yellow-400 animate-pulse">
          ♟ Your turn to move
        </p>
      )}
    </div>
  );
}

// ── Clock overview ────────────────────────────────────────────────────────────

interface ClockOverviewProps {
  games: SimulGame[];
}

function ClockOverview({ games }: ClockOverviewProps) {
  const active = games.filter((g) => !isFinished(g.status));
  const hostTurnCount = active.filter((g) => g.status === "host_turn").length;
  const won = games.filter((g) => g.status === "checkmate_host").length;
  const lost = games.filter((g) => g.status === "checkmate_opponent").length;
  const drawn = games.filter((g) => g.status === "draw").length;

  return (
    <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
      <div className="rounded-xl bg-yellow-900/20 border border-yellow-700/30 p-3 text-center">
        <p className="text-2xl font-bold text-yellow-300">{hostTurnCount}</p>
        <p className="text-xs text-yellow-400">Your turn</p>
      </div>
      <div className="rounded-xl bg-gray-800/60 border border-gray-700/40 p-3 text-center">
        <p className="text-2xl font-bold text-gray-300">{active.length}</p>
        <p className="text-xs text-gray-400">Active</p>
      </div>
      <div className="rounded-xl bg-teal-900/20 border border-teal-700/30 p-3 text-center">
        <p className="text-2xl font-bold text-teal-300">{won}</p>
        <p className="text-xs text-teal-400">Won</p>
      </div>
      <div className="rounded-xl bg-red-900/20 border border-red-700/30 p-3 text-center">
        <p className="text-2xl font-bold text-red-300">{lost}</p>
        <p className="text-xs text-red-400">Lost/Draw: {drawn}</p>
      </div>
    </div>
  );
}

// ── Main component ────────────────────────────────────────────────────────────

interface SimulHostDashboardProps {
  initialGames: SimulGame[];
  hostName?: string;
  /** Called when spectator mode is toggled. */
  onSpectatorToggle?: (isSpectator: boolean) => void;
  className?: string;
}

/**
 * SimulHostDashboard
 *
 * Host interface for simultaneous exhibitions. Renders a responsive grid of
 * mini-boards (2–20), highlights boards where it's the host's turn with
 * countdown urgency styling, and supports keyboard navigation (Space/Tab to
 * cycle active boards).
 *
 * @example
 *   <SimulHostDashboard
 *     initialGames={games}
 *     hostName="GM Magnus"
 *     onSpectatorToggle={setIsSpectator}
 *   />
 */
export function SimulHostDashboard({
  initialGames,
  hostName = "Host",
  onSpectatorToggle,
  className = "",
}: SimulHostDashboardProps) {
  // Validate board count
  const clampedGames = useMemo(
    () => initialGames.slice(0, MAX_BOARDS),
    [initialGames]
  );

  if (clampedGames.length < MIN_BOARDS) {
    throw new Error(`SimulHostDashboard requires at least ${MIN_BOARDS} games`);
  }

  const [state, dispatch] = useReducer(simulReducer, {
    games: clampedGames,
    focusedBoardId: clampedGames[0]?.id ?? null,
    isSpectatorMode: false,
    hostName,
  });

  // Tick clock every second
  useEffect(() => {
    const interval = setInterval(() => {
      dispatch({ type: "TICK" });
    }, 1000);
    return () => clearInterval(interval);
  }, []);

  // Keyboard navigation: Space / Tab = next active board, Shift+Tab = prev
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === " " || (e.key === "Tab" && !e.shiftKey)) {
        e.preventDefault();
        dispatch({ type: "FOCUS_NEXT_ACTIVE" });
      }
      if (e.key === "Tab" && e.shiftKey) {
        e.preventDefault();
        dispatch({ type: "FOCUS_PREV_ACTIVE" });
      }
    },
    []
  );

  useEffect(() => {
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [handleKeyDown]);

  const focusedGame = state.games.find((g) => g.id === state.focusedBoardId);

  const hostTurnBoards = state.games.filter((g) => g.status === "host_turn");
  const hasUrgentBoards = hostTurnBoards.some((g) => g.hostTimeRemaining <= 30);

  const handleToggleSpectator = () => {
    dispatch({ type: "TOGGLE_SPECTATOR" });
    onSpectatorToggle?.(!state.isSpectatorMode);
  };

  return (
    <div
      className={`text-white ${className}`}
      data-testid="simul-host-dashboard"
    >
      {/* Header */}
      <div className="mb-4 flex flex-wrap items-center gap-3">
        <FaChessKnight className="text-xl text-teal-400" />
        <h2 className="bg-gradient-to-r from-teal-400 to-blue-500 bg-clip-text text-xl font-extrabold tracking-tight text-transparent">
          Simul Host: {hostName}
        </h2>
        <span className="rounded-full bg-gray-700 px-2.5 py-1 text-xs text-gray-300">
          {state.games.length} boards
        </span>

        {/* Urgent warning */}
        {hasUrgentBoards && (
          <span className="flex items-center gap-1.5 rounded-full bg-red-900/30 px-2.5 py-1 text-xs font-semibold text-red-400 animate-pulse border border-red-700/40">
            <FaExclamationTriangle className="text-[10px]" />
            Low time on {hostTurnBoards.filter((g) => g.hostTimeRemaining <= 30).length} board(s)!
          </span>
        )}

        {/* Spectator toggle */}
        <button
          type="button"
          onClick={handleToggleSpectator}
          className={`ml-auto rounded-lg px-3 py-1.5 text-xs font-semibold transition-colors
            ${state.isSpectatorMode
              ? "bg-blue-500/20 text-blue-300 hover:bg-blue-500/30 border border-blue-500/40"
              : "bg-gray-700 text-gray-300 hover:bg-gray-600"
            }`}
        >
          {state.isSpectatorMode ? "👁 Spectating" : "Switch to Spectator"}
        </button>
      </div>

      {/* Clock overview */}
      <div className="mb-4">
        <ClockOverview games={state.games} />
      </div>

      <div className="flex flex-col gap-4 lg:flex-row">
        {/* Mini-board grid */}
        <div className="flex-1">
          <div className="mb-2 flex items-center justify-between">
            <p className="text-xs font-semibold uppercase tracking-wider text-gray-400">
              All Boards
            </p>
            <p className="text-[10px] text-gray-600">
              Space / Tab to cycle · Shift+Tab to go back
            </p>
          </div>
          <div
            className="grid gap-2"
            style={{
              gridTemplateColumns: "repeat(auto-fill, minmax(120px, 1fr))",
            }}
            data-testid="simul-board-grid"
          >
            {state.games.map((game) => (
              <MiniTile
                key={game.id}
                game={game}
                isFocused={game.id === state.focusedBoardId}
                isSpectator={state.isSpectatorMode}
                onFocus={(id) => dispatch({ type: "FOCUS_BOARD", id })}
              />
            ))}
          </div>
        </div>

        {/* Focused board */}
        <div className="w-full lg:w-80 xl:w-96 shrink-0">
          {focusedGame ? (
            <FocusedBoard
              game={focusedGame}
              isSpectator={state.isSpectatorMode}
              onMove={(gameId, san, newFen) =>
                dispatch({ type: "MAKE_MOVE", gameId, san, newFen })
              }
            />
          ) : (
            <div className="flex h-64 items-center justify-center rounded-2xl border border-gray-700 bg-gray-900/60 text-sm text-gray-500">
              Select a board to focus
            </div>
          )}

          {/* Result ticker */}
          <div className="mt-3 rounded-xl border border-gray-700 bg-gray-900/60 p-3">
            <h4 className="mb-2 text-xs font-semibold uppercase tracking-wider text-gray-400">
              Result Ticker
            </h4>
            <div className="max-h-40 overflow-y-auto space-y-1">
              {state.games
                .filter((g) => isFinished(g.status))
                .map((g) => (
                  <div
                    key={g.id}
                    className="flex items-center justify-between text-xs"
                  >
                    <span className="text-gray-400">vs {g.opponentName}</span>
                    <span
                      className={`font-semibold ${
                        g.status === "checkmate_host"
                          ? "text-teal-400"
                          : g.status === "draw"
                          ? "text-blue-400"
                          : "text-red-400"
                      }`}
                    >
                      {g.status === "checkmate_host"
                        ? "1–0"
                        : g.status === "draw"
                        ? "½–½"
                        : "0–1"}
                    </span>
                  </div>
                ))}
              {state.games.every((g) => !isFinished(g.status)) && (
                <p className="text-[10px] text-gray-600 text-center">
                  No results yet
                </p>
              )}
            </div>
          </div>
        </div>
      </div>

      {/* Overall result summary */}
      {state.games.every((g) => isFinished(g.status)) && (
        <div className="mt-6 rounded-2xl border border-yellow-500/30 bg-gradient-to-r from-yellow-900/20 to-teal-900/20 p-5 text-center">
          <FaTrophy className="mx-auto mb-2 text-3xl text-yellow-400" />
          <h3 className="text-lg font-bold text-white">Simul Complete!</h3>
          <p className="mt-1 text-sm text-gray-400">
            {state.games.filter((g) => g.status === "checkmate_host").length} wins ·{" "}
            {state.games.filter((g) => g.status === "draw").length} draws ·{" "}
            {state.games.filter((g) => g.status === "checkmate_opponent").length} losses
          </p>
        </div>
      )}
    </div>
  );
}

export default SimulHostDashboard;
