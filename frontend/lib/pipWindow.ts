import type { SpectatorMove } from "@/hook/useSpectatorSocket";

/**
 * Pure geometry + presentation helpers for the spectator Picture-in-Picture
 * (PIP) chessboard windows.
 *
 * Everything here is framework-free so the drag/resize/snap rules can be unit
 * tested without a DOM, and so the component layer only has to wire events to
 * these functions.
 */

export const PIP_MIN_WIDTH = 180;
export const PIP_MAX_WIDTH = 400;
export const PIP_MIN_HEIGHT = PIP_MIN_WIDTH;
export const PIP_MAX_HEIGHT = PIP_MAX_WIDTH;
export const PIP_DEFAULT_WIDTH = 240;
/** Keep at least this much space between a window and the viewport edge. */
export const PIP_VIEWPORT_MARGIN = 8;
/** Distance within which a dragged window snaps flush to an edge. */
export const PIP_EDGE_SNAP_THRESHOLD = 24;
/** Diagonal offset applied when stacking a new window. */
export const PIP_STACK_OFFSET = 28;

export interface PipRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface ViewportSize {
  width: number;
  height: number;
}

export interface PipWindowState {
  id: string;
  gameId: string;
  rect: PipRect;
  zIndex: number;
}

export function clamp(value: number, min: number, max: number): number {
  if (Number.isNaN(value)) return min;
  return Math.min(Math.max(value, min), max);
}

/**
 * Keep a window fully inside the viewport. When the viewport is smaller than
 * the window itself the window is pinned to the top-left margin rather than
 * pushed off-screen with a negative offset.
 */
export function clampRectToViewport(
  rect: PipRect,
  viewport: ViewportSize,
  margin: number = PIP_VIEWPORT_MARGIN,
): PipRect {
  const maxX = Math.max(margin, viewport.width - rect.width - margin);
  const maxY = Math.max(margin, viewport.height - rect.height - margin);
  return {
    ...rect,
    x: clamp(rect.x, margin, maxX),
    y: clamp(rect.y, margin, maxY),
  };
}

/**
 * Snap a window flush to whichever viewport edges it is already close to.
 * Called after a drag so releases near an edge settle cleanly.
 */
export function snapRectToEdges(
  rect: PipRect,
  viewport: ViewportSize,
  threshold: number = PIP_EDGE_SNAP_THRESHOLD,
  margin: number = PIP_VIEWPORT_MARGIN,
): PipRect {
  let { x, y } = rect;
  const rightGap = viewport.width - (x + rect.width) - margin;
  const bottomGap = viewport.height - (y + rect.height) - margin;

  if (Math.abs(x - margin) <= threshold) x = margin;
  else if (Math.abs(rightGap) <= threshold) x = viewport.width - rect.width - margin;

  if (Math.abs(y - margin) <= threshold) y = margin;
  else if (Math.abs(bottomGap) <= threshold) y = viewport.height - rect.height - margin;

  return { ...rect, x, y };
}

/** Apply a pointer delta to a window, then clamp and edge-snap the result. */
export function movePipWindow(
  rect: PipRect,
  deltaX: number,
  deltaY: number,
  viewport: ViewportSize,
  margin: number = PIP_VIEWPORT_MARGIN,
): PipRect {
  const moved = { ...rect, x: rect.x + deltaX, y: rect.y + deltaY };
  return snapRectToEdges(clampRectToViewport(moved, viewport, margin), viewport, PIP_EDGE_SNAP_THRESHOLD, margin);
}

/**
 * Resize a PIP window by its width. The board is square, so height follows
 * width; width is clamped to the 180–400px range from the acceptance criteria
 * and the whole rect is re-clamped to the viewport.
 */
export function resizePipWindow(
  rect: PipRect,
  requestedWidth: number,
  viewport: ViewportSize,
  margin: number = PIP_VIEWPORT_MARGIN,
): PipRect {
  const width = clamp(Math.round(requestedWidth), PIP_MIN_WIDTH, PIP_MAX_WIDTH);
  const resized: PipRect = { ...rect, width, height: width };
  return clampRectToViewport(resized, viewport, margin);
}

/** Bottom-right default slot for the first PIP window. */
export function defaultPipRect(
  viewport: ViewportSize,
  slotIndex = 0,
  width: number = PIP_DEFAULT_WIDTH,
): PipRect {
  const size = clamp(width, PIP_MIN_WIDTH, PIP_MAX_WIDTH);
  const base: PipRect = {
    x: viewport.width - size - PIP_VIEWPORT_MARGIN - slotIndex * PIP_STACK_OFFSET,
    y: viewport.height - size - PIP_VIEWPORT_MARGIN - slotIndex * PIP_STACK_OFFSET,
    width: size,
    height: size,
  };
  return snapRectToEdges(clampRectToViewport(base, viewport), viewport);
}

/** Next free stack slot, walking up-and-left from the bottom-right corner. */
export function nextStackSlot(
  existing: PipWindowState[],
  viewport: ViewportSize,
  width: number = PIP_DEFAULT_WIDTH,
): PipRect {
  return defaultPipRect(viewport, existing.length, width);
}

/** Highest z-index in the current set (0 when empty). */
export function topZIndex(windows: PipWindowState[]): number {
  return windows.reduce((max, window) => Math.max(max, window.zIndex), 0);
}

/** Raise a window above the rest without reordering the array. */
export function bringToFront(windows: PipWindowState[], gameId: string): PipWindowState[] {
  return windows.map((window) =>
    window.gameId === gameId ? { ...window, zIndex: topZIndex(windows) + 1 } : window,
  );
}

/**
 * Open a PIP window for a game. Opening a game that already has a window just
 * raises it, so the same game can never be pinned twice.
 */
export function openPipWindow(
  windows: PipWindowState[],
  gameId: string,
  viewport: ViewportSize,
  options: { width?: number } = {},
): PipWindowState[] {
  if (windows.some((window) => window.gameId === gameId)) {
    return bringToFront(windows, gameId);
  }
  return [
    ...windows,
    {
      id: `pip-${gameId}`,
      gameId,
      rect: nextStackSlot(windows, viewport, options.width),
      zIndex: topZIndex(windows) + 1,
    },
  ];
}

export function closePipWindow(windows: PipWindowState[], gameId: string): PipWindowState[] {
  return windows.filter((window) => window.gameId !== gameId);
}

export function updatePipRect(
  windows: PipWindowState[],
  gameId: string,
  rect: PipRect,
): PipWindowState[] {
  return windows.map((window) => (window.gameId === gameId ? { ...window, rect } : window));
}

export function isPointInRect(x: number, y: number, rect: PipRect): boolean {
  return x >= rect.x && x <= rect.x + rect.width && y >= rect.y && y <= rect.y + rect.height;
}

// ─── Chess presentation helpers ───────────────────────────────────────────────

const PIECE_SYMBOLS: Record<string, string> = {
  k: "♚",
  q: "♛",
  r: "♜",
  b: "♝",
  n: "♞",
  p: "♟",
  K: "♔",
  Q: "♕",
  R: "♖",
  B: "♗",
  N: "♘",
  P: "♙",
};

const PIECE_VALUES: Record<string, number> = {
  p: 1,
  n: 3,
  b: 3,
  r: 5,
  q: 9,
};

const STARTING_FEN = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

function emptyBoardRows(): string[][] {
  return Array.from({ length: 8 }, () => Array.from({ length: 8 }, () => ""));
}

/**
 * Expand a FEN placement field into 8 rows of 8 glyphs. Unknown input
 * degrades to an empty board rather than throwing, since a malformed frame
 * from the spectator socket should never take the PIP window down.
 */
export function fenToBoardRows(fen: string | null | undefined): string[][] {
  if (!fen) return emptyBoardRows();
  const placement = fen.trim().split(/\s+/)[0];
  const ranks = placement.split("/");
  if (ranks.length !== 8) return emptyBoardRows();

  const rows: string[][] = [];
  for (const rank of ranks) {
    const row: string[] = [];
    for (const char of rank) {
      if (/[1-8]/.test(char)) {
        const blanks = Number(char);
        for (let i = 0; i < blanks; i += 1) row.push("");
      } else if (PIECE_SYMBOLS[char]) {
        row.push(PIECE_SYMBOLS[char]);
      } else {
        return emptyBoardRows();
      }
    }
    if (row.length !== 8) return emptyBoardRows();
    rows.push(row);
  }
  return rows;
}

/**
 * Cheap material evaluation in centipawns, used for the PIP mini eval bar.
 * A full engine search is intentionally out of scope: this is a zero-cost
 * heuristic that keeps multiple PIP windows off the CPU/GPU budget.
 */
export function materialBalanceCp(fen: string | null | undefined): number {
  const placement = (fen ?? STARTING_FEN).trim().split(/\s+/)[0];
  let score = 0;
  for (const char of placement) {
    const value = PIECE_VALUES[char.toLowerCase()];
    if (!value) continue;
    score += char === char.toLowerCase() ? -value : value;
  }
  return score * 100;
}

/** White-share percentage (0–100) for the mini evaluation bar. */
export function evaluationPercent(centipawns: number, maxCp = 1000): number {
  const clamped = clamp(centipawns, -maxCp, maxCp);
  return clamp(50 + (clamped / maxCp) * 50, 0, 100);
}

/** Human-readable evaluation, e.g. `+1.2`, `-0.5`, `0.0`, `#`. */
export function formatEvaluation(centipawns: number): string {
  if (Math.abs(centipawns) >= 10000) return "#";
  const pawns = centipawns / 100;
  const sign = pawns > 0 ? "+" : pawns < 0 ? "-" : "";
  return `${sign}${Math.abs(pawns).toFixed(1)}`;
}

/**
 * Compact move label for the PIP notification line, e.g. `12...Nf6`.
 * `plyIndex` is 0-based (0 = White's first move).
 */
export function moveNotificationText(move: SpectatorMove, plyIndex: number): string {
  const moveNumber = Math.floor(Math.max(0, plyIndex) / 2) + 1;
  const separator = move.color === "w" ? "." : "...";
  return `${moveNumber}${separator}${move.san}`;
}

export const PIP_STARTING_FEN = STARTING_FEN;
