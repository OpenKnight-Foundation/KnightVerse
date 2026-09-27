"use client";

import React, {
  useState,
  useRef,
  useCallback,
  useEffect,
  useMemo,
  WheelEvent,
  MouseEvent as ReactMouseEvent,
  TouchEvent as ReactTouchEvent,
} from "react";
import { FaTrophy, FaChessKnight, FaEye } from "react-icons/fa";

// ── Types ─────────────────────────────────────────────────────────────────────

export type MatchStatus =
  | "Upcoming"
  | "Ongoing"
  | "Checkmate"
  | "Draw"
  | "Forfeit"
  | "Pending";

export type TournamentStatus = "Registration" | "InProgress" | "Completed";
export type BracketFormat = "SingleElimination" | "DoubleElimination" | "Swiss";

export interface Participant {
  id: string;
  wallet_address: string;
  display_name: string;
  elo: number;
  seed: number;
}

export interface BracketMatch {
  id: string;
  /** Round number (1-indexed). */
  round: number;
  match_number: number;
  player1_id: string | null;
  player2_id: string | null;
  winner_id: string | null;
  status: MatchStatus;
  /** ISO timestamp or null. */
  scheduled_at: string | null;
  completed_at: string | null;
  /** Whether this match is the user's own active match. */
  isUserMatch?: boolean;
}

export interface TournamentBracket {
  id: string;
  name: string;
  format: BracketFormat;
  status: TournamentStatus;
  participants: Participant[];
  matches: BracketMatch[];
  total_rounds: number;
  winner_id: string | null;
  created_at: string;
  started_at: string | null;
  completed_at: string | null;
}

interface TournamentBracketViewerProps {
  bracket: TournamentBracket;
  /**
   * Current viewer's participant ID. Used to auto-scroll to the user's match
   * and highlight it.
   */
  currentUserId?: string;
  /**
   * Called when the user clicks "Spectate" on an ongoing match.
   * Receives the match ID.
   */
  onSpectate?: (matchId: string) => void;
  className?: string;
}

// ── Constants ─────────────────────────────────────────────────────────────────

/** Card pixel dimensions */
const CARD_W = 176;
const CARD_H = 72;
const COL_GAP = 56;
const ROW_BASE_GAP = 16;

const STATUS_STYLES: Record<MatchStatus, string> = {
  Upcoming:  "border-gray-700/50 bg-gray-800/40",
  Pending:   "border-gray-700/50 bg-gray-800/40",
  Ongoing:   "border-yellow-500/60 bg-yellow-900/15 animate-pulse",
  Checkmate: "border-teal-500/50 bg-teal-900/10",
  Draw:      "border-blue-500/50 bg-blue-900/10",
  Forfeit:   "border-red-500/40 bg-red-900/10",
};

const STATUS_BADGE: Record<MatchStatus, string> = {
  Upcoming:  "bg-gray-700 text-gray-400",
  Pending:   "bg-gray-700 text-gray-400",
  Ongoing:   "bg-yellow-500/20 text-yellow-300 border border-yellow-500/40",
  Checkmate: "bg-teal-500/20 text-teal-300 border border-teal-500/40",
  Draw:      "bg-blue-500/20 text-blue-300 border border-blue-500/40",
  Forfeit:   "bg-red-500/20 text-red-300 border border-red-500/40",
};

const TOURNAMENT_STATUS_BADGE: Record<TournamentStatus, string> = {
  Registration: "bg-blue-500/20 text-blue-300 border-blue-500/30",
  InProgress:   "bg-yellow-500/20 text-yellow-300 border-yellow-500/30",
  Completed:    "bg-teal-500/20 text-teal-300 border-teal-500/30",
};

// ── Helpers ───────────────────────────────────────────────────────────────────

function getParticipant(
  bracket: TournamentBracket,
  id: string | null
): Participant | undefined {
  if (!id) return undefined;
  return bracket.participants.find((p) => p.id === id);
}

function roundLabel(round: number, totalRounds: number): string {
  if (round === totalRounds) return "Final";
  if (round === totalRounds - 1) return "Semi-finals";
  if (round === totalRounds - 2 && totalRounds > 3) return "Quarter-finals";
  return `Round ${round}`;
}

// ── MatchCard ─────────────────────────────────────────────────────────────────

interface MatchCardProps {
  match: BracketMatch;
  bracket: TournamentBracket;
  onSpectate?: (matchId: string) => void;
  isUserMatch: boolean;
  cardRef?: React.Ref<HTMLDivElement>;
}

function MatchCard({ match, bracket, onSpectate, isUserMatch, cardRef }: MatchCardProps) {
  const p1 = getParticipant(bracket, match.player1_id);
  const p2 = getParticipant(bracket, match.player2_id);

  const rowClass = (playerId: string | null) => {
    if (!playerId) return "text-gray-600 italic";
    if (match.winner_id === playerId) return "text-teal-300 font-bold";
    if (match.status === "Checkmate" || match.status === "Forfeit")
      return "text-gray-500 line-through";
    return "text-white";
  };

  const canSpectate = match.status === "Ongoing" && onSpectate;

  return (
    <div
      ref={cardRef}
      className={`relative rounded-xl border p-2.5 text-xs transition-all duration-200
        ${STATUS_STYLES[match.status]}
        ${isUserMatch ? "ring-2 ring-indigo-500 ring-offset-1 ring-offset-gray-900" : ""}
      `}
      style={{ width: CARD_W, minHeight: CARD_H }}
      data-testid={`match-card-${match.id}`}
    >
      {/* Status badge */}
      <div className="mb-1.5 flex items-center justify-between gap-1">
        <span className={`rounded-full px-1.5 py-0.5 text-[10px] font-semibold ${STATUS_BADGE[match.status]}`}>
          {match.status}
        </span>
        {isUserMatch && (
          <span className="rounded-full bg-indigo-500/20 px-1.5 py-0.5 text-[10px] font-semibold text-indigo-300">
            Your match
          </span>
        )}
      </div>

      {/* Players */}
      <div className={`truncate border-b border-gray-700/40 py-0.5 ${rowClass(match.player1_id)}`}>
        {p1 ? `#${p1.seed} ${p1.display_name}` : "TBD"}
      </div>
      <div className={`truncate py-0.5 ${rowClass(match.player2_id)}`}>
        {p2 ? `#${p2.seed} ${p2.display_name}` : "TBD"}
      </div>

      {/* Spectate button */}
      {canSpectate && (
        <button
          type="button"
          onClick={() => onSpectate(match.id)}
          className="absolute right-2 top-2 flex items-center gap-1 rounded-lg bg-yellow-500/20 px-1.5 py-0.5
                     text-[10px] font-semibold text-yellow-300 hover:bg-yellow-500/40 transition-colors"
          aria-label={`Spectate match ${match.id}`}
        >
          <FaEye className="text-[9px]" />
          Watch
        </button>
      )}
    </div>
  );
}

// ── Swiss standings table ─────────────────────────────────────────────────────

function SwissStandings({ bracket }: { bracket: TournamentBracket }) {
  const wins: Record<string, number> = {};
  for (const m of bracket.matches) {
    if (m.winner_id) wins[m.winner_id] = (wins[m.winner_id] ?? 0) + 1;
  }

  const rows = bracket.participants
    .map((p) => ({ participant: p, wins: wins[p.id] ?? 0 }))
    .sort((a, b) => b.wins - a.wins || a.participant.seed - b.participant.seed);

  return (
    <div className="overflow-x-auto">
      <table className="w-full min-w-[360px] text-sm text-left">
        <thead>
          <tr className="border-b border-gray-700 text-xs uppercase tracking-wider text-gray-400">
            <th className="py-2 pr-4">Rank</th>
            <th className="py-2 pr-4">Player</th>
            <th className="py-2 pr-4">ELO</th>
            <th className="py-2">Wins</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r, i) => (
            <tr
              key={r.participant.id}
              className={`border-b border-gray-800 ${
                bracket.winner_id === r.participant.id ? "text-teal-300" : "text-gray-300"
              }`}
            >
              <td className="py-2 pr-4 font-mono">
                {bracket.winner_id === r.participant.id && (
                  <FaTrophy className="mr-1 inline text-yellow-400" />
                )}
                {i + 1}
              </td>
              <td className="py-2 pr-4 font-semibold">{r.participant.display_name}</td>
              <td className="py-2 pr-4 text-gray-400">{r.participant.elo}</td>
              <td className="py-2 font-mono">{r.wins}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

// ── Pan-and-zoom bracket canvas ───────────────────────────────────────────────

interface BracketCanvasProps {
  bracket: TournamentBracket;
  currentUserId?: string;
  onSpectate?: (matchId: string) => void;
}

function BracketCanvas({ bracket, currentUserId, onSpectate }: BracketCanvasProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const innerRef = useRef<HTMLDivElement>(null);

  // transform state
  const [offset, setOffset] = useState({ x: 32, y: 32 });
  const [scale, setScale] = useState(1);
  const dragging = useRef(false);
  const lastPos = useRef({ x: 0, y: 0 });
  // touch pinch
  const lastPinchDist = useRef<number | null>(null);

  // ref for the user's active match card
  const userMatchRef = useRef<HTMLDivElement | null>(null);

  const rounds = useMemo(
    () => Array.from(new Set(bracket.matches.map((m) => m.round))).sort((a, b) => a - b),
    [bracket.matches]
  );

  // Group matches by round
  const byRound = useMemo(() => {
    const map: Record<number, BracketMatch[]> = {};
    for (const m of bracket.matches) {
      (map[m.round] ??= []).push(m);
    }
    // Sort each round by match_number
    for (const r of Object.keys(map)) {
      map[Number(r)].sort((a, b) => a.match_number - b.match_number);
    }
    return map;
  }, [bracket.matches]);

  // Find user's match
  const userMatch = useMemo(
    () =>
      currentUserId
        ? bracket.matches.find(
            (m) =>
              (m.player1_id === currentUserId || m.player2_id === currentUserId) &&
              (m.status === "Ongoing" || m.status === "Upcoming")
          )
        : undefined,
    [bracket.matches, currentUserId]
  );

  // Auto-scroll to user's active match
  useEffect(() => {
    if (!userMatchRef.current || !containerRef.current) return;
    const card = userMatchRef.current;
    const container = containerRef.current;
    const cardRect = card.getBoundingClientRect();
    const containerRect = container.getBoundingClientRect();
    const targetX = -(cardRect.left - containerRect.left - containerRect.width / 2 + CARD_W / 2) + offset.x;
    const targetY = -(cardRect.top - containerRect.top - containerRect.height / 2 + CARD_H / 2) + offset.y;
    setOffset({ x: targetX, y: targetY });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [userMatch?.id]);

  // ── Drag pan ──
  const onMouseDown = useCallback((e: ReactMouseEvent) => {
    dragging.current = true;
    lastPos.current = { x: e.clientX, y: e.clientY };
  }, []);

  const onMouseMove = useCallback((e: ReactMouseEvent) => {
    if (!dragging.current) return;
    const dx = e.clientX - lastPos.current.x;
    const dy = e.clientY - lastPos.current.y;
    lastPos.current = { x: e.clientX, y: e.clientY };
    setOffset((o) => ({ x: o.x + dx, y: o.y + dy }));
  }, []);

  const onMouseUp = useCallback(() => {
    dragging.current = false;
  }, []);

  // ── Wheel zoom ──
  const onWheel = useCallback((e: WheelEvent) => {
    e.preventDefault();
    const delta = e.deltaY > 0 ? 0.9 : 1.1;
    setScale((s) => Math.max(0.3, Math.min(2.5, s * delta)));
  }, []);

  // ── Touch pan / pinch-zoom ──
  const onTouchStart = useCallback((e: ReactTouchEvent) => {
    if (e.touches.length === 1) {
      dragging.current = true;
      lastPos.current = { x: e.touches[0].clientX, y: e.touches[0].clientY };
    } else if (e.touches.length === 2) {
      const dx = e.touches[0].clientX - e.touches[1].clientX;
      const dy = e.touches[0].clientY - e.touches[1].clientY;
      lastPinchDist.current = Math.hypot(dx, dy);
    }
  }, []);

  const onTouchMove = useCallback((e: ReactTouchEvent) => {
    if (e.touches.length === 1 && dragging.current) {
      const dx = e.touches[0].clientX - lastPos.current.x;
      const dy = e.touches[0].clientY - lastPos.current.y;
      lastPos.current = { x: e.touches[0].clientX, y: e.touches[0].clientY };
      setOffset((o) => ({ x: o.x + dx, y: o.y + dy }));
    } else if (e.touches.length === 2 && lastPinchDist.current !== null) {
      const dx = e.touches[0].clientX - e.touches[1].clientX;
      const dy = e.touches[0].clientY - e.touches[1].clientY;
      const dist = Math.hypot(dx, dy);
      const ratio = dist / lastPinchDist.current;
      lastPinchDist.current = dist;
      setScale((s) => Math.max(0.3, Math.min(2.5, s * ratio)));
    }
  }, []);

  const onTouchEnd = useCallback(() => {
    dragging.current = false;
    lastPinchDist.current = null;
  }, []);

  // ── Zoom controls ──
  const zoomIn = () => setScale((s) => Math.min(2.5, s * 1.2));
  const zoomOut = () => setScale((s) => Math.max(0.3, s / 1.2));
  const resetView = () => { setScale(1); setOffset({ x: 32, y: 32 }); };

  return (
    <div className="relative">
      {/* Zoom controls */}
      <div className="absolute right-3 top-3 z-10 flex flex-col gap-1">
        <button
          type="button"
          onClick={zoomIn}
          aria-label="Zoom in"
          className="flex h-7 w-7 items-center justify-center rounded-lg bg-gray-800 text-white hover:bg-gray-700 text-base font-bold shadow"
        >
          +
        </button>
        <button
          type="button"
          onClick={zoomOut}
          aria-label="Zoom out"
          className="flex h-7 w-7 items-center justify-center rounded-lg bg-gray-800 text-white hover:bg-gray-700 text-base font-bold shadow"
        >
          −
        </button>
        <button
          type="button"
          onClick={resetView}
          aria-label="Reset view"
          className="flex h-7 w-7 items-center justify-center rounded-lg bg-gray-800 text-gray-400 hover:bg-gray-700 text-xs shadow"
        >
          ⌂
        </button>
      </div>

      {/* Scrollable canvas */}
      <div
        ref={containerRef}
        className="relative overflow-hidden rounded-xl border border-gray-800 bg-gray-950"
        style={{ height: 480, touchAction: "none", userSelect: "none", cursor: dragging.current ? "grabbing" : "grab" }}
        onMouseDown={onMouseDown}
        onMouseMove={onMouseMove}
        onMouseUp={onMouseUp}
        onMouseLeave={onMouseUp}
        onWheel={onWheel}
        onTouchStart={onTouchStart}
        onTouchMove={onTouchMove}
        onTouchEnd={onTouchEnd}
      >
        <div
          ref={innerRef}
          style={{
            transform: `translate(${offset.x}px, ${offset.y}px) scale(${scale})`,
            transformOrigin: "0 0",
            display: "flex",
            gap: COL_GAP,
            alignItems: "flex-start",
            padding: 8,
          }}
        >
          {rounds.map((round, colIdx) => {
            const matches = byRound[round] ?? [];
            // Vertical gap grows exponentially per round for single-elimination
            const rowGap =
              bracket.format === "DoubleElimination"
                ? ROW_BASE_GAP * 2
                : ROW_BASE_GAP * Math.pow(2, colIdx);

            return (
              <div key={round} style={{ display: "flex", flexDirection: "column", gap: rowGap, alignItems: "center" }}>
                <span
                  className="mb-2 text-[10px] font-semibold uppercase tracking-widest text-gray-500"
                  style={{ width: CARD_W, textAlign: "center" }}
                >
                  {roundLabel(round, bracket.total_rounds)}
                </span>
                {matches.map((m) => {
                  const isUserMatch = !!(
                    currentUserId &&
                    (m.player1_id === currentUserId || m.player2_id === currentUserId)
                  );
                  return (
                    <MatchCard
                      key={m.id}
                      match={m}
                      bracket={bracket}
                      onSpectate={onSpectate}
                      isUserMatch={isUserMatch}
                      cardRef={isUserMatch && m.id === userMatch?.id ? userMatchRef : undefined}
                    />
                  );
                })}
              </div>
            );
          })}
        </div>
      </div>

      <p className="mt-1.5 text-center text-[10px] text-gray-600">
        Drag to pan · Scroll / pinch to zoom
      </p>
    </div>
  );
}

// ── Main component ────────────────────────────────────────────────────────────

/**
 * TournamentBracketViewer
 *
 * Responsive, pan-and-zoom capable bracket visualization supporting Single
 * Elimination, Double Elimination, and Swiss formats with real-time match
 * status indicators and click-to-spectate integration.
 *
 * @example
 *   <TournamentBracketViewer
 *     bracket={tournamentData}
 *     currentUserId={myParticipantId}
 *     onSpectate={(matchId) => router.push(`/watch/${matchId}`)}
 *   />
 */
export function TournamentBracketViewer({
  bracket,
  currentUserId,
  onSpectate,
  className = "",
}: TournamentBracketViewerProps) {
  const isElimination =
    bracket.format === "SingleElimination" || bracket.format === "DoubleElimination";

  return (
    <div className={`text-white ${className}`} data-testid="tournament-bracket-viewer">
      {/* Header */}
      <div className="mb-5 flex flex-wrap items-center gap-3">
        <FaChessKnight className="text-xl text-teal-400" />
        <h2 className="bg-gradient-to-r from-teal-400 to-blue-500 bg-clip-text text-2xl font-extrabold tracking-tight text-transparent">
          {bracket.name}
        </h2>
        <span
          className={`rounded-full border px-3 py-1 text-xs font-semibold ${TOURNAMENT_STATUS_BADGE[bracket.status]}`}
        >
          {bracket.status}
        </span>
        <span className="text-xs text-gray-500">
          {bracket.format.replace(/([A-Z])/g, " $1").trim()}
        </span>
        <span className="ml-auto text-xs text-gray-500">
          {bracket.participants.length} players
        </span>
      </div>

      {/* Champion banner */}
      {bracket.status === "Completed" && bracket.winner_id && (
        <div className="mb-5 flex items-center gap-3 rounded-2xl border border-yellow-500/30 bg-gradient-to-r from-yellow-500/10 to-teal-500/10 p-4">
          <FaTrophy className="text-2xl text-yellow-400" />
          <div>
            <p className="text-xs uppercase tracking-widest text-gray-400">Champion</p>
            <p className="text-lg font-bold text-yellow-300">
              {getParticipant(bracket, bracket.winner_id)?.display_name ?? "Unknown"}
            </p>
          </div>
        </div>
      )}

      {/* Bracket or standings */}
      {isElimination ? (
        <BracketCanvas
          bracket={bracket}
          currentUserId={currentUserId}
          onSpectate={onSpectate}
        />
      ) : (
        <SwissStandings bracket={bracket} />
      )}

      {/* Participants */}
      <div className="mt-8">
        <h3 className="mb-3 text-xs font-semibold uppercase tracking-widest text-gray-400">
          Participants ({bracket.participants.length})
        </h3>
        <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-4">
          {bracket.participants.map((p) => (
            <div
              key={p.id}
              className={`flex items-center gap-2 rounded-lg border bg-gray-800/50 px-3 py-2 text-xs
                ${currentUserId === p.id ? "border-indigo-500/60" : "border-gray-700/40"}`}
            >
              <span className="w-5 font-mono text-gray-500">#{p.seed}</span>
              <span className="truncate font-medium text-white">{p.display_name}</span>
              <span className="ml-auto text-gray-400">{p.elo}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

export default TournamentBracketViewer;
