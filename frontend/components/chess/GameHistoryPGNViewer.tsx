"use client";

import { useState, useEffect, useRef, useCallback } from "react";
import { Chess } from "chess.js";
import ChessboardComponent from "./ChessboardComponent";
import { downloadScoresheetPdf } from "@/lib/scoresheetPdf";

// ── Mock PGN ──────────────────────────────────────────────────────────────────

/**
 * Mock PGN for a complete game with clock annotations.
 * Replace with the real PGN string from the API when available.
 * Clock comments use the standard %clk format: { [%clk h:mm:ss] }
 */
export const MOCK_PGN = `[Event "KnightVerse Rated Game"]
[Site "knightverse.app"]
[Date "2026.03.26"]
[White "GABC...XYZ"]
[Black "GDEF...UVW"]
[Result "1-0"]
[WhiteElo "1280"]
[BlackElo "1263"]
[TimeControl "300+3"]

1. e4 { [%clk 0:05:00] } e5 { [%clk 0:05:00] }
2. Nf3 { [%clk 0:04:58] } d6 { [%clk 0:04:57] }
3. d4 { [%clk 0:04:55] } Bg4 { [%clk 0:04:54] }
4. dxe5 { [%clk 0:04:52] } Bxf3 { [%clk 0:04:51] }
5. Qxf3 { [%clk 0:04:50] } dxe5 { [%clk 0:04:49] }
6. Bc4 { [%clk 0:04:47] } Nf6 { [%clk 0:04:46] }
7. Qb3 { [%clk 0:04:45] } Qe7 { [%clk 0:04:43] }
8. Nc3 { [%clk 0:04:43] } c6 { [%clk 0:04:41] }
9. Bg5 { [%clk 0:04:41] } b5 { [%clk 0:04:39] }
10. Nxb5 { [%clk 0:04:38] } cxb5 { [%clk 0:04:37] }
11. Bxb5+ { [%clk 0:04:36] } Nbd7 { [%clk 0:04:35] }
12. O-O-O { [%clk 0:04:34] } Rd8 { [%clk 0:04:33] }
13. Rxd7 { [%clk 0:04:32] } Rxd7 { [%clk 0:04:30] }
14. Rd1 { [%clk 0:04:30] } Qe6 { [%clk 0:04:28] }
15. Bxd7+ { [%clk 0:04:28] } Nxd7 { [%clk 0:04:26] }
16. Qb8+ { [%clk 0:04:26] } Nxb8 { [%clk 0:04:24] }
17. Rd8# { [%clk 0:04:24] } 1-0`;

// ── Types ─────────────────────────────────────────────────────────────────────

interface ParsedMove {
  san: string;
  from: string;
  to: string;
  piece: string;
  captured?: string;
  color: "w" | "b";
  /** White's clock after this move (if present in PGN). */
  whiteClock?: string;
  /** Black's clock after this move (if present in PGN). */
  blackClock?: string;
  /** Move number e.g. 1, 1, 2, 2 ... */
  moveNumber: number;
}

interface CapturedPieces {
  white: string[]; // pieces captured by white (black pieces taken)
  black: string[]; // pieces captured by black (white pieces taken)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

const PIECE_SYMBOLS: Record<string, string> = {
  p: "♟", r: "♜", n: "♞", b: "♝", q: "♛", k: "♚",
  P: "♙", R: "♖", N: "♘", B: "♗", Q: "♕", K: "♔",
};

const PIECE_LABEL: Record<string, string> = {
  p: "Pawn", r: "Rook", n: "Knight",
  b: "Bishop", q: "Queen", k: "King",
};

/**
 * Extract all %clk annotations from a PGN string in order.
 * PGN clock format: { [%clk h:mm:ss] }
 * Returns an array of clock strings in move order.
 */
function extractClocks(pgn: string): string[] {
  const matches = pgn.match(/\[%clk\s+([\d:]+)\]/g) ?? [];
  return matches.map((m) => m.replace(/\[%clk\s+/, "").replace("]", "").trim());
}

/**
 * Parse a PGN string into a list of FEN positions and annotated moves.
 * Returns null if the PGN is invalid.
 */
function parsePGN(pgn: string): {
  fens: string[];
  moves: ParsedMove[];
  headers: Record<string, string>;
} | null {
  try {
    const chess = new Chess();
    chess.loadPgn(pgn);

    const verboseMoves = chess.history({ verbose: true });
    const clocks = extractClocks(pgn);

    // Re-play from start to collect FEN at every position
    chess.reset();
    const fens: string[] = [chess.fen()]; // index 0 = starting position

    const moves: ParsedMove[] = verboseMoves.map((m, i) => {
      chess.move(m.san);
      fens.push(chess.fen());

      // Clocks arrive in pairs: even indices = white, odd = black
      const whiteClock = m.color === "w" ? clocks[i] : undefined;
      const blackClock = m.color === "b" ? clocks[i] : undefined;

      return {
        san: m.san,
        from: m.from,
        to: m.to,
        piece: m.piece,
        captured: m.captured,
        color: m.color,
        whiteClock,
        blackClock,
        moveNumber: Math.ceil((i + 1) / 2),
      };
    });

    // Extract PGN headers
    const headers: Record<string, string> = {};
    const headerMatches = pgn.matchAll(/\[(\w+)\s+"([^"]+)"\]/g);
    for (const [, key, value] of headerMatches) {
      headers[key] = value;
    }

    return { fens, moves, headers };
  } catch {
    return null;
  }
}

/**
 * Accumulate captured pieces up to a given move index.
 * Index 0 = no moves played yet.
 */
function getCapturedPieces(moves: ParsedMove[], upToIndex: number): CapturedPieces {
  const result: CapturedPieces = { white: [], black: [] };
  for (let i = 0; i < upToIndex && i < moves.length; i++) {
    const m = moves[i];
    if (m.captured) {
      if (m.color === "w") {
        result.white.push(m.captured); // white captured a black piece
      } else {
        result.black.push(m.captured); // black captured a white piece
      }
    }
  }
  return result;
}

// ── Sub-components ────────────────────────────────────────────────────────────

function CapturedRow({
  label,
  pieces,
  color,
}: {
  label: string;
  pieces: string[];
  color: "white" | "black";
}) {
  if (pieces.length === 0) return null;
  return (
    <div className="flex items-center gap-2">
      <span className="text-xs text-gray-400 w-12 shrink-0">{label}</span>
      <div className="flex flex-wrap gap-0.5">
        {pieces.map((p, i) => (
          <span
            key={i}
            title={PIECE_LABEL[p] ?? p}
            className={`text-lg leading-none ${
              color === "white" ? "text-gray-200" : "text-gray-800"
            }`}
          >
            {color === "white"
              ? PIECE_SYMBOLS[p.toUpperCase()]
              : PIECE_SYMBOLS[p.toLowerCase()]}
          </span>
        ))}
      </div>
    </div>
  );
}

function PlaybackButton({
  onClick,
  disabled,
  label,
  children,
}: {
  onClick: () => void;
  disabled: boolean;
  label: string;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      aria-label={label}
      className="flex items-center justify-center w-10 h-10 rounded-lg
                 bg-gray-700 text-white font-mono text-sm font-bold
                 hover:bg-indigo-600 disabled:opacity-30 disabled:cursor-not-allowed
                 transition-colors focus:outline-none focus-visible:ring-2
                 focus-visible:ring-indigo-400"
    >
      {children}
    </button>
  );
}

// ── Main component ────────────────────────────────────────────────────────────

interface GameHistoryPGNViewerProps {
  /**
   * PGN string to replay. Defaults to MOCK_PGN.
   * Swap for a real PGN from the API once available.
   */
  pgn?: string;
}

/**
 * GameHistoryPGNViewer
 *
 * Replay a past game by stepping through its PGN move history.
 * Controls: << (start), < (back), > (forward), >> (end).
 *
 * Displays:
 * - The board position at the current move
 * - Captured pieces for both sides
 * - Clock timestamps extracted from PGN %clk annotations
 * - Scrollable move list with the current move highlighted
 *
 * @example
 *   <GameHistoryPGNViewer />                  // mock PGN
 *   <GameHistoryPGNViewer pgn={game.pgn} />   // real PGN
 */
export function GameHistoryPGNViewer({ pgn = MOCK_PGN }: GameHistoryPGNViewerProps) {
  const [currentIndex, setCurrentIndex] = useState(0);
  const [parsed, setParsed] = useState<ReturnType<typeof parsePGN>>(null);
  const [parseError, setParseError] = useState(false);
  const moveListRef = useRef<HTMLDivElement>(null);

  // Parse PGN once on mount or when pgn prop changes
  useEffect(() => {
    const result = parsePGN(pgn);
    if (!result) {
      setParseError(true);
      return;
    }
    setParsed(result);
    setCurrentIndex(0);
    setParseError(false);
  }, [pgn]);

  // Scroll the active move into view in the move list
  useEffect(() => {
    if (!moveListRef.current) return;
    const active = moveListRef.current.querySelector("[data-active='true']");
    active?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }, [currentIndex]);

  const totalPositions = parsed?.fens.length ?? 1;
  const atStart = currentIndex === 0;
  const atEnd   = currentIndex === totalPositions - 1;

  const goToStart   = useCallback(() => setCurrentIndex(0), []);
  const goBack      = useCallback(() => setCurrentIndex((i) => Math.max(0, i - 1)), []);
  const goForward   = useCallback(() => setCurrentIndex((i) => Math.min(totalPositions - 1, i + 1)), [totalPositions]);
  const goToEnd     = useCallback(() => setCurrentIndex(totalPositions - 1), [totalPositions]);

  // Keyboard navigation
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "ArrowLeft")  { e.preventDefault(); goBack(); }
      if (e.key === "ArrowRight") { e.preventDefault(); goForward(); }
      if (e.key === "Home")       { e.preventDefault(); goToStart(); }
      if (e.key === "End")        { e.preventDefault(); goToEnd(); }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [goBack, goForward, goToStart, goToEnd]);

  if (parseError) {
    return (
      <div className="rounded-xl border border-red-800 bg-gray-900 p-6 text-center">
        <p className="text-red-400 font-medium">Invalid PGN — could not parse game.</p>
      </div>
    );
  }

  if (!parsed) {
    return (
      <div className="rounded-xl border border-gray-700 bg-gray-900 p-6 text-center">
        <p className="text-gray-400">Loading game…</p>
      </div>
    );
  }

  const { fens, moves, headers } = parsed;
  const currentFen = fens[currentIndex];

  const handleExportPdf = (): void => {
    downloadScoresheetPdf({
      metadata: {
        white: headers.White ?? "White",
        black: headers.Black ?? "Black",
        whiteElo: headers.WhiteElo,
        blackElo: headers.BlackElo,
        date: headers.Date,
        result: headers.Result,
        event: headers.Event,
        site: headers.Site,
        timeControl: headers.TimeControl,
      },
      moves: moves.map((move) => move.san),
      finalFen: fens[fens.length - 1],
    });
  };

  // The move that brought us to currentIndex (index 0 = no move yet)
  const currentMove = currentIndex > 0 ? moves[currentIndex - 1] : null;
  const captured = getCapturedPieces(moves, currentIndex);

  // Clock for the current position — from the move that just played
  const clock = currentMove?.color === "w"
    ? currentMove.whiteClock
    : currentMove?.blackClock;

  return (
    <div className="rounded-xl border border-gray-700 bg-gray-900 text-white overflow-hidden">

      {/* Header */}
      <div className="px-5 py-4 border-b border-gray-700">
        <div className="flex items-center justify-between gap-3">
          <h2 className="text-sm font-semibold text-white">Game Replay</h2>
          <button
            type="button"
            onClick={handleExportPdf}
            aria-label="Export this game as a PDF scoresheet"
            className="inline-flex items-center rounded-lg px-3 py-1.5 text-xs font-semibold
                       bg-indigo-600 text-white hover:bg-indigo-500
                       transition-colors focus:outline-none focus-visible:ring-2
                       focus-visible:ring-indigo-400"
          >
            Export as PDF
          </button>
        </div>
        <div className="flex flex-wrap gap-x-4 gap-y-0.5 mt-2 text-xs text-gray-400">
          {headers.White && <span>⬜ {headers.White}</span>}
          {headers.Black && <span>⬛ {headers.Black}</span>}
          {headers.Result && (
            <span className="text-indigo-400 font-semibold">{headers.Result}</span>
          )}
          {headers.Date && <span>{headers.Date.replace(/\./g, "-")}</span>}
        </div>
      </div>

      <div className="flex flex-col lg:flex-row gap-0">

        {/* Board + controls */}
        <div className="flex flex-col items-center p-5 gap-4 lg:border-r lg:border-gray-700">

          {/* Captured by black (white pieces taken) */}
          <div className="w-full max-w-[400px] min-h-[28px]">
            <CapturedRow label="Taken" pieces={captured.black} color="white" />
          </div>

          {/* Board — read-only, onDrop is a no-op */}
          <div className="w-full max-w-[400px]">
            <ChessboardComponent
              position={currentFen}
              onDrop={() => false}
            />
          </div>

          {/* Captured by white (black pieces taken) */}
          <div className="w-full max-w-[400px] min-h-[28px]">
            <CapturedRow label="Taken" pieces={captured.white} color="black" />
          </div>

          {/* Clock */}
          <div className="h-5 text-xs text-gray-400">
            {clock && (
              <span>
                🕐 {currentMove?.color === "w" ? "White" : "Black"}: {clock}
              </span>
            )}
          </div>

          {/* Playback controls */}
          <div
            className="flex items-center gap-2"
            role="group"
            aria-label="Playback controls"
          >
            <PlaybackButton onClick={goToStart} disabled={atStart} label="Go to start">
              &#171;&#171;
            </PlaybackButton>
            <PlaybackButton onClick={goBack} disabled={atStart} label="Previous move">
              &#8249;
            </PlaybackButton>

            <span className="text-xs text-gray-400 px-2 min-w-[56px] text-center">
              {currentIndex === 0
                ? "Start"
                : `${currentMove?.moveNumber}${currentMove?.color === "w" ? "." : "…"} ${currentMove?.san}`}
            </span>

            <PlaybackButton onClick={goForward} disabled={atEnd} label="Next move">
              &#8250;
            </PlaybackButton>
            <PlaybackButton onClick={goToEnd} disabled={atEnd} label="Go to end">
              &#187;&#187;
            </PlaybackButton>
          </div>

          <p className="text-xs text-gray-600">
            Use ← → arrow keys to step through moves
          </p>
        </div>

        {/* Move list */}
        <div className="flex flex-col flex-1 min-w-0">
          <div className="px-4 py-3 border-b border-gray-700">
            <h3 className="text-xs font-semibold uppercase tracking-wide text-gray-400">
              Move List
            </h3>
          </div>

          <div
            ref={moveListRef}
            className="overflow-y-auto max-h-[420px] p-2"
            aria-label="Move list"
          >
            {/* Group moves into pairs (white + black per row) */}
            {Array.from(
              { length: Math.ceil(moves.length / 2) },
              (_, i) => {
                const whiteMove = moves[i * 2];
                const blackMove = moves[i * 2 + 1];
                const whiteMoveIndex = i * 2 + 1; // position index in fens[]
                const blackMoveIndex = i * 2 + 2;

                return (
                  <div
                    key={i}
                    className="grid grid-cols-[28px_1fr_1fr] gap-1 rounded px-1"
                  >
                    {/* Move number */}
                    <span className="text-xs text-gray-500 py-1.5 text-right">
                      {i + 1}.
                    </span>

                    {/* White move */}
                    <button
                      type="button"
                      data-active={currentIndex === whiteMoveIndex}
                      onClick={() => setCurrentIndex(whiteMoveIndex)}
                      className={`text-left text-sm px-2 py-1.5 rounded transition-colors
                        ${currentIndex === whiteMoveIndex
                          ? "bg-indigo-600 text-white font-semibold"
                          : "text-gray-300 hover:bg-gray-700"
                        }`}
                    >
                      {whiteMove?.san}
                      {whiteMove?.whiteClock && (
                        <span className="ml-1 text-xs text-gray-400 font-normal">
                          {whiteMove.whiteClock}
                        </span>
                      )}
                    </button>

                    {/* Black move */}
                    {blackMove ? (
                      <button
                        type="button"
                        data-active={currentIndex === blackMoveIndex}
                        onClick={() => setCurrentIndex(blackMoveIndex)}
                        className={`text-left text-sm px-2 py-1.5 rounded transition-colors
                          ${currentIndex === blackMoveIndex
                            ? "bg-indigo-600 text-white font-semibold"
                            : "text-gray-300 hover:bg-gray-700"
                          }`}
                      >
                        {blackMove.san}
                        {blackMove.blackClock && (
                          <span className="ml-1 text-xs text-gray-400 font-normal">
                            {blackMove.blackClock}
                          </span>
                        )}
                      </button>
                    ) : (
                      <span />
                    )}
                  </div>
                );
              }
            )}
          </div>

          {/* Move count footer */}
          <div className="px-4 py-3 border-t border-gray-700 mt-auto">
            <p className="text-xs text-gray-500">
              {currentIndex} / {moves.length} moves
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}