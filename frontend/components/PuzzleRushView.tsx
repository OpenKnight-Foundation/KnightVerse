"use client";

import dynamic from "next/dynamic";
import { useEffect, useMemo, useRef, useState } from "react";
import { ArrowLeft, Flame, Play, RotateCcw, Trophy, X } from "lucide-react";
import { Chess } from "chess.js";

const ChessboardComponent = dynamic(() => import("@/components/chess/ChessboardComponent"), { ssr: false });
type RushMode = "three" | "five" | "survival";
type RushPuzzle = { id: string; fen: string; solution: string };
type PuzzleRushViewProps = {
  onExit: () => void;
  onVerifyMove?: (puzzleId: string, move: { from: string; to: string }) => Promise<boolean>;
  /** Known leaderboard rows; the host owns the API. */
  leaderboardEntries?: LeaderboardEntry[];
  currentHandle?: string;
  /** Persist a finished run. Rejections are ignored so the UI stays optimistic. */
  onSubmitScore?: (entry: LeaderboardEntry) => Promise<unknown>;
};
const puzzles: RushPuzzle[] = [
  { id: "rush-1", fen: "r1bqkbnr/pppp1ppp/2n5/1B2p3/4P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 0 4", solution: "f1e1" },
  { id: "rush-2", fen: "rnbqkb1r/pppp1ppp/5n2/2B1p3/4P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 0 4", solution: "f1e1" },
  { id: "rush-3", fen: "rnbqkbnr/pp1ppppp/2p5/3p4/3P4/2N5/PP1PPPPP/R1BQKBNR w KQkq - 0 3", solution: "e4d5" },
];
const durations: Record<RushMode, number> = { three: 180, five: 300, survival: 0 };


// ---------------------------------------------------------------------------
// Shared types: leaderboard entries, trophy tiers, pending offline runs (#1117)
// ---------------------------------------------------------------------------

type LeaderboardScope = "today" | "week" | "all";
type TrophyTier = "gold" | "silver" | "bronze";

type LeaderboardEntry = {
  id: string;
  handle: string;
  /** Stellar public key, shown truncated; optional when a handle is enough. */
  publicKey?: string;
  score: number;
  bestStreak: number;
  solved: number;
  attempted: number;
  /** Average milliseconds per solve, used for the speed distribution. */
  avgSolveMs: number;
  achievedAt: string;
  mode: RushMode;
  /** True for the signed-in player's own row. */
  isYou?: boolean;
};

const TIER_THRESHOLDS: Record<TrophyTier, number> = { gold: 3000, silver: 1500, bronze: 0 };

/** Trophy tier is derived from score so the three never disagree. */
export function trophyTierFor(score: number): TrophyTier {
  if (score >= TIER_THRESHOLDS.gold) return "gold";
  if (score >= TIER_THRESHOLDS.silver) return "silver";
  return "bronze";
}

const TIER_STYLES: Record<TrophyTier, string> = {
  gold: "border-yellow-400/60 bg-yellow-400/10 text-yellow-200",
  silver: "border-slate-300/50 bg-slate-300/10 text-slate-200",
  bronze: "border-orange-700/60 bg-orange-700/15 text-orange-300",
};

export function accuracyOf(entry: Pick<LeaderboardEntry, "solved" | "attempted">): number {
  if (entry.attempted <= 0) return 0;
  return Math.round((entry.solved / entry.attempted) * 100);
}

function shortenKey(key: string): string {
  return key.length <= 12 ? key : `${key.slice(0, 6)}…${key.slice(-4)}`;
}

/** Case-insensitive match on handle or Stellar public key. */
export function matchesQuery(entry: LeaderboardEntry, query: string): boolean {
  const term = query.trim().toLowerCase();
  if (!term) return true;
  return (
    entry.handle.toLowerCase().includes(term) ||
    (entry.publicKey ?? "").toLowerCase().includes(term)
  );
}

const SCOPE_WINDOW_MS: Record<LeaderboardScope, number> = {
  today: 24 * 60 * 60 * 1000,
  week: 7 * 24 * 60 * 60 * 1000,
  all: Number.POSITIVE_INFINITY,
};

/** Apply the time filter, then the search term, then sort by score. */
export function filterLeaderboard(
  entries: LeaderboardEntry[],
  scope: LeaderboardScope,
  query: string,
  now: number = Date.now(),
): LeaderboardEntry[] {
  const cutoff = now - SCOPE_WINDOW_MS[scope];
  return entries
    .filter((entry) => Date.parse(entry.achievedAt) >= cutoff)
    .filter((entry) => matchesQuery(entry, query))
    .sort((a, b) => b.score - a.score || a.handle.localeCompare(b.handle));
}

const LEADERBOARD_PAGE_SIZE = 5;

/** Clamp the page so a stale or out-of-range page never renders empty. */
export function paginate<T>(items: T[], page: number, pageSize: number = LEADERBOARD_PAGE_SIZE) {
  const size = Math.max(1, pageSize);
  const pageCount = Math.max(1, Math.ceil(items.length / size));
  const current = Math.min(Math.max(1, Math.floor(page) || 1), pageCount);
  const start = (current - 1) * size;
  return {
    items: items.slice(start, start + size),
    page: current,
    pageCount,
    total: items.length,
    hasPrevious: current > 1,
    hasNext: current < pageCount,
  };
}

// ---------------------------------------------------------------------------
// Offline support (#1123)
// ---------------------------------------------------------------------------

/** Track connectivity so a run can be played and verified without a server. */
export function useOnlineStatus(): boolean {
  const [online, setOnline] = useState(true);

  useEffect(() => {
    if (typeof navigator === "undefined") return;
    setOnline(navigator.onLine);
    const goOnline = () => setOnline(true);
    const goOffline = () => setOnline(false);
    window.addEventListener("online", goOnline);
    window.addEventListener("offline", goOffline);
    return () => {
      window.removeEventListener("online", goOnline);
      window.removeEventListener("offline", goOffline);
    };
  }, []);

  return online;
}

/**
 * Offline move check against the fixture's declared solution.
 *
 * This is a local approximation, not an engine evaluation: online runs still
 * go to `onVerifyMove`, which stays authoritative.
 */
export function matchesDeclaredSolution(
  puzzle: RushPuzzle,
  move: { from: string; to: string; promotion?: string },
): boolean {
  const expected = puzzle.solution;
  const played = `${move.from}${move.to}${move.promotion ?? ""}`;
  return expected === played || expected === `${move.from}${move.to}`;
}

const PENDING_SCORES_KEY = "knightverse.puzzle-rush.pending";

/** Runs finished offline, waiting to be replayed to the server. */
export function readPendingScores(): LeaderboardEntry[] {
  if (typeof window === "undefined") return [];
  try {
    const raw = window.localStorage.getItem(PENDING_SCORES_KEY);
    if (!raw) return [];
    const parsed: unknown = JSON.parse(raw);
    return Array.isArray(parsed) ? (parsed as LeaderboardEntry[]) : [];
  } catch {
    return [];
  }
}

export function writePendingScores(entries: LeaderboardEntry[]): void {
  if (typeof window === "undefined") return;
  try {
    if (entries.length === 0) window.localStorage.removeItem(PENDING_SCORES_KEY);
    else window.localStorage.setItem(PENDING_SCORES_KEY, JSON.stringify(entries));
  } catch {
    // A full or unavailable localStorage must not break a finished run.
  }
}

export interface PuzzleLeaderboardProps {
  entries: LeaderboardEntry[];
  /** Highlight and pin the signed-in player's row. */
  currentHandle?: string;
  className?: string;
}

/**
 * High-score leaderboard with mode tabs, time filters, search and pagination
 * (#1117). Presented as a presentational component: filtering, search and
 * paging are computed here, the data itself is supplied by the host.
 */
export function PuzzleLeaderboard({ entries, currentHandle, className = "" }: PuzzleLeaderboardProps) {
  const [mode, setMode] = useState<RushMode>("three");
  const [scope, setScope] = useState<LeaderboardScope>("week");
  const [query, setQuery] = useState("");
  const [page, setPage] = useState(1);

  const forMode = useMemo(
    () => filterLeaderboard(entries, scope, query).filter((entry) => entry.mode === mode),
    [entries, scope, query, mode],
  );
  const paged = paginate(forMode, page);
  const you = entries.find((entry) => entry.isYou || entry.handle === currentHandle);

  const switchMode = (next: RushMode) => {
    setMode(next);
    setPage(1);
  };

  return (
    <section className={`border border-slate-700 bg-slate-900 p-6 ${className}`} aria-label="Puzzle Rush leaderboard">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h2 className="flex items-center gap-2 text-lg font-bold">
          <Trophy size={18} className="text-yellow-300" />
          High scores
        </h2>
        <label className="flex items-center gap-2 text-xs text-slate-400">
          Search
          <input
            type="search"
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
              setPage(1);
            }}
            placeholder="handle or public key"
            aria-label="Search leaderboard by handle or public key"
            className="border border-slate-700 bg-slate-950 px-3 py-1.5 text-sm text-white"
          />
        </label>
      </div>

      <div className="mt-4 flex flex-wrap gap-2" role="tablist" aria-label="Rush mode">
        {(["three", "five", "survival"] as RushMode[]).map((item) => (
          <button
            key={item}
            role="tab"
            aria-selected={mode === item}
            onClick={() => switchMode(item)}
            className={`px-3 py-1.5 text-xs font-semibold ${
              mode === item ? "bg-orange-300 text-slate-950" : "border border-slate-700 text-slate-300"
            }`}
          >
            {item === "survival" ? "Survival" : `${item === "three" ? "3" : 5}-Minute`}
          </button>
        ))}
      </div>

      <div className="mt-3 flex flex-wrap gap-2" role="group" aria-label="Time range">
        {(["today", "week", "all"] as LeaderboardScope[]).map((item) => (
          <button
            key={item}
            onClick={() => {
              setScope(item);
              setPage(1);
            }}
            aria-pressed={scope === item}
            className={`px-2.5 py-1 text-xs ${
              scope === item ? "border border-orange-300 text-orange-300" : "border border-slate-800 text-slate-500"
            }`}
          >
            {item === "today" ? "Today" : item === "week" ? "This Week" : "All-Time"}
          </button>
        ))}
      </div>

      {you && (
        <div className="mt-4 border border-slate-800 bg-slate-950 p-4" data-testid="rush-personal-stats">
          <p className="text-xs uppercase tracking-[0.2em] text-slate-500">Your run</p>
          <p className="mt-1 text-2xl font-bold text-orange-300">{you.score.toLocaleString()}</p>
          <dl className="mt-3 grid grid-cols-3 gap-3 text-xs">
            <div>
              <dt className="text-slate-500">Accuracy</dt>
              <dd className="text-sm font-semibold text-white">{accuracyOf(you)}%</dd>
            </div>
            <div>
              <dt className="text-slate-500">Best streak</dt>
              <dd className="text-sm font-semibold text-white">{you.bestStreak}x</dd>
            </div>
            <div>
              <dt className="text-slate-500">Avg solve</dt>
              <dd className="text-sm font-semibold text-white">{(you.avgSolveMs / 1000).toFixed(1)}s</dd>
            </div>
          </dl>
        </div>
      )}

      {paged.total === 0 ? (
        <p className="mt-6 border border-dashed border-slate-700 p-6 text-center text-sm text-slate-500">
          No runs recorded for this mode and range yet.
        </p>
      ) : (
        <ol className="mt-4 flex flex-col gap-2">
          {paged.items.map((entry) => {
            const tier = trophyTierFor(entry.score);
            const isYou = entry.isYou || entry.handle === currentHandle;
            return (
              <li
                key={entry.id}
                className={`flex items-center gap-3 border p-3 ${TIER_STYLES[tier]} ${
                  isYou ? "ring-1 ring-orange-300" : ""
                }`}
              >
                <span
                  className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-slate-800 text-xs font-bold text-white"
                  aria-hidden="true"
                >
                  {entry.handle.slice(0, 2).toUpperCase()}
                </span>
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm font-semibold text-white">
                    {entry.handle}
                    {isYou && <span className="ml-2 text-xs text-orange-300">you</span>}
                  </p>
                  <p className="truncate text-xs text-slate-400">
                    {entry.publicKey ? shortenKey(entry.publicKey) : "unverified handle"}
                  </p>
                </div>
                <div className="text-right">
                  <p className="text-sm font-bold text-white">{entry.score.toLocaleString()}</p>
                  <p className="text-xs capitalize text-slate-400">
                    {tier} · {entry.bestStreak}x
                  </p>
                </div>
              </li>
            );
          })}
        </ol>
      )}

      {paged.pageCount > 1 && (
        <nav className="mt-4 flex items-center justify-center gap-3" aria-label="Leaderboard pagination">
          <button
            onClick={() => setPage((value) => Math.max(1, value - 1))}
            disabled={!paged.hasPrevious}
            className="border border-slate-700 px-3 py-1.5 text-xs text-slate-300 disabled:opacity-40"
          >
            Previous
          </button>
          <span className="text-xs text-slate-500">
            Page {paged.page} of {paged.pageCount}
          </span>
          <button
            onClick={() => setPage((value) => Math.min(paged.pageCount, value + 1))}
            disabled={!paged.hasNext}
            className="border border-slate-700 px-3 py-1.5 text-xs text-slate-300 disabled:opacity-40"
          >
            Next
          </button>
        </nav>
      )}
    </section>
  );
}

export default function PuzzleRushView({ onExit, onVerifyMove, leaderboardEntries = [], currentHandle, onSubmitScore }: PuzzleRushViewProps) {
  const [mode, setMode] = useState<RushMode | null>(null);
  const [timeLeft, setTimeLeft] = useState(0);
  const [score, setScore] = useState(0);
  const [streak, setStreak] = useState(0);
  const [strikes, setStrikes] = useState(0);
  const [puzzleIndex, setPuzzleIndex] = useState(0);
  const [fen, setFen] = useState("");
  const [feedback, setFeedback] = useState<"correct" | "incorrect" | null>(null);
  const [finished, setFinished] = useState(false);
  const isOnline = useOnlineStatus();
  /** Wall-clock time the current puzzle was shown, for solve-speed stats. */
  const puzzleStartedAtRef = useRef<number>(0);
  /** Guards against recording the same finished run twice. */
  const recordedRef = useRef(false);
  /** Per-run statistics, reset by start(). */
  const [solveTimes, setSolveTimes] = useState<number[]>([]);
  const [bestStreak, setBestStreak] = useState(0);
  /** Rows produced by this browser, shown optimistically before any sync. */
  const [localRows, setLocalRows] = useState<LeaderboardEntry[]>([]);
  const [pendingCount, setPendingCount] = useState(0);
  /** Mode-select screen shows either the mode grid or the leaderboard. */
  const [showBoard, setShowBoard] = useState(false);
  const gameRef = useRef(new Chess());
  const currentPuzzle = puzzles[puzzleIndex % puzzles.length];

  const start = (nextMode: RushMode) => { setMode(nextMode); setTimeLeft(durations[nextMode]); setScore(0); setStreak(0); setStrikes(0); setPuzzleIndex(0); setFinished(false); setBestStreak(0); setSolveTimes([]); recordedRef.current = false; puzzleStartedAtRef.current = Date.now(); gameRef.current.load(puzzles[0].fen); setFen(gameRef.current.fen()); };
  useEffect(() => { if (!mode || finished || mode === "survival") return; const timer = window.setInterval(() => setTimeLeft((value) => { if (value <= 1) { setFinished(true); return 0; } return value - 1; }), 1000); return () => window.clearInterval(timer); }, [mode, finished]);
  const finishIfNeeded = (nextStrikes: number) => { if (mode === "survival" && nextStrikes >= 3) setFinished(true); };
  const playTone = (frequency: number) => { if (typeof window === "undefined") return; const context = new AudioContext(); const oscillator = context.createOscillator(); const gain = context.createGain(); oscillator.frequency.value = frequency; gain.gain.value = 0.04; oscillator.connect(gain); gain.connect(context.destination); oscillator.start(); oscillator.stop(context.currentTime + 0.12); };
  const handleMove = async ({ sourceSquare, targetSquare }: { sourceSquare: string; targetSquare: string }) => {
    if (!mode || finished) return false;
    try { const move = gameRef.current.move({ from: sourceSquare, to: targetSquare, promotion: "q" }); if (!move) return false; const correct = onVerifyMove && isOnline ? await onVerifyMove(currentPuzzle.id, { from: sourceSquare, to: targetSquare }) : matchesDeclaredSolution(currentPuzzle, { from: sourceSquare, to: targetSquare, promotion: "q" }); if (correct) { const nextStreak = streak + 1; setStreak(nextStreak); setBestStreak((value) => Math.max(value, nextStreak)); setSolveTimes((times) => [...times, Date.now() - puzzleStartedAtRef.current]); setScore((value) => value + 100 * Math.max(1, nextStreak)); setFeedback("correct"); playTone(880); window.setTimeout(() => { const next = (puzzleIndex + 1) % puzzles.length; setPuzzleIndex(next); gameRef.current.load(puzzles[next].fen); setFen(gameRef.current.fen()); setFeedback(null); }, 120); } else { const nextStrikes = strikes + 1; setStrikes(nextStrikes); setStreak(0); setFeedback("incorrect"); playTone(160); finishIfNeeded(nextStrikes); gameRef.current.load(currentPuzzle.fen); setFen(gameRef.current.fen()); } return true; } catch { return false; }
  };
  const allRows = useMemo(
    () => [...localRows, ...leaderboardEntries.filter((row) => !localRows.some((mine) => mine.id === row.id))],
    [localRows, leaderboardEntries],
  );

  // Record the run exactly once when it finishes.
  useEffect(() => {
    if (!finished || !mode || recordedRef.current) return;
    recordedRef.current = true;
    setPendingCount(readPendingScores().length);

    const solved = solveTimes.length;
    const attempted = solved + strikes;
    const entry: LeaderboardEntry = {
      id: `local-${mode}-${Date.now()}`,
      handle: currentHandle ?? "You",
      score,
      bestStreak,
      solved,
      attempted,
      avgSolveMs: solved > 0 ? solveTimes.reduce((total, ms) => total + ms, 0) / solved : 0,
      achievedAt: new Date().toISOString(),
      mode,
      isYou: true,
    };

    // Optimistic: the row shows immediately, before any network round trip.
    setLocalRows((rows) => [entry, ...rows]);

    if (!onSubmitScore) return;
    if (!isOnline) {
      const pending = readPendingScores();
      writePendingScores([...pending, entry]);
      setPendingCount(pending.length + 1);
      return;
    }
    void Promise.resolve(onSubmitScore(entry)).catch(() => undefined);
  }, [finished, mode, score, bestStreak, solveTimes, strikes, currentHandle, isOnline, onSubmitScore]);

  // Flush anything recorded offline once connectivity returns.
  useEffect(() => {
    if (!isOnline || !onSubmitScore) return;
    const pending = readPendingScores();
    if (pending.length === 0) return;
    void Promise.all(pending.map((row) => Promise.resolve(onSubmitScore(row)).catch(() => row)))
      .then((results) => {
        const failed = results.filter((row): row is LeaderboardEntry => typeof row !== "undefined");
        writePendingScores(failed);
        setPendingCount(failed.length);
      });
  }, [isOnline, onSubmitScore]);

  const formatTime = (value: number) => `${Math.floor(value / 60)}:${String(value % 60).padStart(2, "0")}`;

  if (!mode) return <main className="min-h-screen bg-slate-950 px-4 py-12 text-white"><div className="mx-auto max-w-4xl"><button onClick={onExit} className="mb-10 flex items-center gap-2 text-sm text-slate-400 hover:text-white"><ArrowLeft size={16} />Back to puzzles</button><div className="mb-10 max-w-2xl"><p className="mb-3 text-sm uppercase tracking-[0.25em] text-orange-300">Arcade training</p><h1 className="text-5xl font-bold tracking-tight">Puzzle Rush</h1><p className="mt-4 text-slate-400">Solve continuously. Build a streak. Three strikes ends survival.</p></div><div className="mb-6 flex gap-2" role="tablist" aria-label="Puzzle Rush sections">
        <button role="tab" aria-selected={!showBoard} onClick={() => setShowBoard(false)} className={`px-3 py-1.5 text-xs font-semibold ${!showBoard ? "bg-orange-300 text-slate-950" : "border border-slate-700 text-slate-300"}`}>Play</button>
        <button role="tab" aria-selected={showBoard} onClick={() => setShowBoard(true)} className={`px-3 py-1.5 text-xs font-semibold ${showBoard ? "bg-orange-300 text-slate-950" : "border border-slate-700 text-slate-300"}`}>Leaderboard</button>
      </div>
      {showBoard ? <PuzzleLeaderboard entries={allRows} currentHandle={currentHandle} /> : <div className="grid gap-4 md:grid-cols-3">{(["three", "five", "survival"] as RushMode[]).map((item) => <button key={item} onClick={() => start(item)} className="border border-slate-700 bg-slate-900 p-6 text-left transition hover:-translate-y-1 hover:border-orange-300"><p className="text-lg font-bold">{item === "survival" ? "Survival" : `${item === "three" ? "3" : "5"}-Minute`}</p><p className="mt-2 text-sm text-slate-400">{item === "survival" ? "Three strikes" : "Race the clock"}</p><span className="mt-8 flex items-center gap-2 text-sm text-orange-300"><Play size={15} />Start run</span></button>)}</div>}</div></main>;
  if (finished) return <main className="min-h-screen bg-slate-950 px-4 py-12 text-white"><div className="mx-auto max-w-xl border border-slate-700 bg-slate-900 p-8 text-center"><Trophy className="mx-auto mb-4 text-yellow-300" size={42} /><p className="text-sm uppercase tracking-[0.2em] text-orange-300">Run complete</p><h1 className="mt-2 text-4xl font-bold">Final score {score.toLocaleString()}</h1><p className="mt-3 text-slate-400">{puzzleIndex} puzzles solved with a best streak of {streak}.</p><PuzzleLeaderboard entries={allRows} currentHandle={currentHandle} className="mt-8 text-left" /><div className="mt-8 flex gap-3"><button onClick={() => setMode(null)} className="flex-1 border border-slate-600 px-4 py-3 text-sm">Choose mode</button><button onClick={() => start(mode)} className="flex-1 bg-orange-300 px-4 py-3 text-sm font-bold text-slate-950"><RotateCcw className="mr-2 inline" size={16} />Replay</button></div></div></main>;
  return <main className="min-h-screen bg-slate-950 px-4 py-8 text-white"><div className="mx-auto max-w-6xl">{!isOnline && (
        <div
          role="status"
          aria-live="polite"
          data-testid="rush-offline-banner"
          className="mb-4 flex flex-wrap items-center justify-between gap-2 border border-amber-500/50 bg-amber-500/10 px-4 py-2 text-sm text-amber-200"
        >
          <span>
            You are offline. This run is played and checked locally &mdash; it will sync when you reconnect.
          </span>
          {pendingCount > 0 && (
            <span className="text-xs text-amber-300/80">
              {pendingCount} run{pendingCount === 1 ? "" : "s"} waiting to sync
            </span>
          )}
        </div>
      )}<div className="mb-6 flex items-center justify-between"><button onClick={onExit} aria-label="Exit Puzzle Rush" className="p-2 text-slate-400 hover:text-white"><X size={20} /></button><div className="flex items-center gap-6 text-sm"><span>Score <strong className="text-orange-300">{score.toLocaleString()}</strong></span><span className="flex items-center gap-1"><Flame size={16} className="text-orange-300" />{streak}x</span><span>{mode === "survival" ? `${3 - strikes} strikes` : formatTime(timeLeft)}</span></div></div><div className="mb-6 h-2 bg-slate-800">{mode !== "survival" && <div className="h-full bg-orange-300 transition-[width]" style={{ width: `${(timeLeft / durations[mode]) * 100}%` }} />}</div><div className="mx-auto max-w-[560px]"><ChessboardComponent position={fen} onDrop={handleMove} /><p className={`mt-4 min-h-6 text-center text-sm ${feedback === "correct" ? "text-emerald-300" : "text-red-300"}`}>{feedback === "correct" ? "Correct. Next position..." : feedback === "incorrect" ? "Incorrect move. The position has been reset." : "Find the best move"}</p></div></div></main>;
}