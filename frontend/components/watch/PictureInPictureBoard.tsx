"use client";

import { useMemo, useRef } from "react";
import { GripHorizontal, Maximize2, Radio, X } from "lucide-react";

import type { SpectatorMove } from "@/hook/useSpectatorSocket";
import {
  evaluationPercent,
  fenToBoardRows,
  formatEvaluation,
  materialBalanceCp,
  moveNotificationText,
  movePipWindow,
  resizePipWindow,
  type PipRect,
  type ViewportSize,
} from "@/lib/pipWindow";
import { truncateAddress } from "@/lib/spectatorUtils";
import { cn } from "@/lib/utils";

const TITLEBAR_HEIGHT = 28;
const EVAL_BAR_HEIGHT = 4;
const MOVE_LINE_HEIGHT = 18;
const BOARD_CHROME = TITLEBAR_HEIGHT + EVAL_BAR_HEIGHT + MOVE_LINE_HEIGHT;
const KEYBOARD_MOVE_STEP = 10;

export interface PictureInPictureBoardProps {
  gameId: string;
  rect: PipRect;
  zIndex: number;
  viewport: ViewportSize;
  isActive?: boolean;
  white?: { address: string; elo: number };
  black?: { address: string; elo: number };
  fen?: string | null;
  moves?: SpectatorMove[];
  /** Centipawn evaluation; falls back to a material count when omitted. */
  evaluationCp?: number | null;
  /** Connection/round status surfaced in the title bar. */
  status?: string;
  onFocus: (gameId: string) => void;
  onClose: (gameId: string) => void;
  onExpand: (gameId: string) => void;
  onRectChange: (gameId: string, rect: PipRect) => void;
}

/**
 * Floating, draggable and resizable Picture-in-Picture chessboard.
 *
 * Rendering is deliberately cheap — a CSS grid of Unicode glyphs plus a
 * material-based eval bar — so several PIP windows can be pinned at once
 * without touching the engine worker pool or a canvas.
 */
export function PictureInPictureBoard({
  gameId,
  rect,
  zIndex,
  viewport,
  isActive = false,
  white,
  black,
  fen,
  moves = [],
  evaluationCp,
  status = "connecting",
  onFocus,
  onClose,
  onExpand,
  onRectChange,
}: PictureInPictureBoardProps) {
  const dragState = useRef<{ startX: number; startY: number; originX: number; originY: number } | null>(null);
  const resizeState = useRef<{ startX: number; originWidth: number } | null>(null);

  const rows = useMemo(() => fenToBoardRows(fen), [fen]);
  const lastMove = moves.length > 0 ? moves[moves.length - 1] : null;
  const lastMoveText = lastMove ? moveNotificationText(lastMove, moves.length - 1) : null;
  const centipawns = evaluationCp ?? materialBalanceCp(fen);
  const whiteShare = evaluationPercent(centipawns);
  const boardSize = Math.max(60, Math.min(rect.width - 12, rect.height - BOARD_CHROME));

  const label = white && black
    ? `${truncateAddress(white.address)} vs ${truncateAddress(black.address)}`
    : gameId;

  function startDrag(event: React.PointerEvent<HTMLDivElement>) {
    event.preventDefault();
    onFocus(gameId);
    dragState.current = {
      startX: event.clientX,
      startY: event.clientY,
      originX: rect.x,
      originY: rect.y,
    };
    event.currentTarget.setPointerCapture?.(event.pointerId);
  }

  function startResize(event: React.PointerEvent<HTMLSpanElement>) {
    event.preventDefault();
    event.stopPropagation();
    onFocus(gameId);
    resizeState.current = { startX: event.clientX, originWidth: rect.width };
    event.currentTarget.setPointerCapture?.(event.pointerId);
  }

  function handlePointerMove(event: React.PointerEvent<HTMLDivElement>) {
    if (dragState.current) {
      const { startX, startY, originX, originY } = dragState.current;
      onRectChange(
        gameId,
        movePipWindow(
          { ...rect, x: originX, y: originY },
          event.clientX - startX,
          event.clientY - startY,
          viewport,
        ),
      );
      return;
    }

    if (resizeState.current) {
      const { startX, originWidth } = resizeState.current;
      onRectChange(gameId, resizePipWindow(rect, originWidth + (event.clientX - startX), viewport));
    }
  }

  function endPointerInteraction() {
    dragState.current = null;
    resizeState.current = null;
  }

  function handleKeyDown(event: React.KeyboardEvent<HTMLDivElement>) {
    const step = KEYBOARD_MOVE_STEP;
    switch (event.key) {
      case "ArrowLeft":
        event.preventDefault();
        onRectChange(gameId, movePipWindow(rect, -step, 0, viewport));
        break;
      case "ArrowRight":
        event.preventDefault();
        onRectChange(gameId, movePipWindow(rect, step, 0, viewport));
        break;
      case "ArrowUp":
        event.preventDefault();
        onRectChange(gameId, movePipWindow(rect, 0, -step, viewport));
        break;
      case "ArrowDown":
        event.preventDefault();
        onRectChange(gameId, movePipWindow(rect, 0, step, viewport));
        break;
      case "Escape":
        event.preventDefault();
        onClose(gameId);
        break;
      case "Enter":
        event.preventDefault();
        onExpand(gameId);
        break;
      default:
        break;
    }
  }

  return (
    <div
      role="dialog"
      aria-label={`Picture-in-picture board for game ${gameId}`}
      aria-modal={false}
      tabIndex={0}
      data-testid={`pip-window-${gameId}`}
      onPointerDown={() => onFocus(gameId)}
      onPointerMove={handlePointerMove}
      onPointerUp={endPointerInteraction}
      onPointerCancel={endPointerInteraction}
      onKeyDown={handleKeyDown}
      className={cn(
        "pointer-events-auto fixed overflow-hidden rounded-xl border bg-gray-900/95 shadow-2xl backdrop-blur transition-shadow",
        isActive ? "border-teal-500/60 shadow-teal-900/30" : "border-gray-700/50",
      )}
      style={{ left: rect.x, top: rect.y, width: rect.width, height: rect.height, zIndex }}
    >
      <div
        data-testid={`pip-drag-handle-${gameId}`}
        onPointerDown={startDrag}
        className="flex cursor-grab items-center justify-between gap-1 border-b border-gray-700/40 bg-gray-800/80 px-2 active:cursor-grabbing"
        style={{ height: TITLEBAR_HEIGHT }}
      >
        <span className="flex min-w-0 items-center gap-1 text-[10px] uppercase tracking-wider text-gray-400">
          <GripHorizontal className="h-3 w-3 shrink-0" aria-hidden="true" />
          <span className="truncate" title={label}>{label}</span>
        </span>
        <span className="flex shrink-0 items-center gap-0.5">
          <span
            className="inline-flex items-center text-emerald-400"
            role="status"
            aria-label={`Feed ${status}`}
          >
            <Radio className="h-3 w-3" aria-hidden="true" />
          </span>
          <button
            type="button"
            onClick={() => onExpand(gameId)}
            aria-label={`Expand game ${gameId}`}
            className="rounded p-0.5 text-gray-400 hover:bg-gray-700/60 hover:text-white"
          >
            <Maximize2 className="h-3 w-3" />
          </button>
          <button
            type="button"
            onClick={() => onClose(gameId)}
            aria-label={`Close picture-in-picture for game ${gameId}`}
            className="rounded p-0.5 text-gray-400 hover:bg-gray-700/60 hover:text-white"
          >
            <X className="h-3 w-3" />
          </button>
        </span>
      </div>

      <div className="flex w-full justify-center py-1">
        <div
          data-testid={`pip-board-${gameId}`}
          className="grid grid-cols-8 grid-rows-8 overflow-hidden rounded"
          style={{ width: boardSize, height: boardSize, fontSize: Math.max(10, boardSize / 9) }}
        >
          {rows.map((row, rowIndex) =>
            row.map((glyph, colIndex) => (
              <div
                key={`${rowIndex}-${colIndex}`}
                aria-hidden="true"
                className={cn(
                  "flex items-center justify-center leading-none",
                  (rowIndex + colIndex) % 2 === 0 ? "bg-slate-300" : "bg-slate-600",
                )}
              >
                {glyph}
              </div>
            )),
          )}
        </div>
      </div>

      <div
        data-testid={`pip-eval-bar-${gameId}`}
        role="meter"
        aria-label={`Evaluation for game ${gameId}`}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(whiteShare)}
        className="w-full bg-gray-700"
        style={{ height: EVAL_BAR_HEIGHT }}
      >
        <div className="h-full bg-white" style={{ width: `${whiteShare}%` }} />
      </div>

      <div
        data-testid={`pip-move-${gameId}`}
        className="flex items-center justify-between gap-2 px-2 text-[10px] text-gray-300"
        style={{ height: MOVE_LINE_HEIGHT }}
      >
        <span className="truncate">{lastMoveText ?? "Waiting for moves…"}</span>
        <span className="shrink-0 tabular-nums text-gray-400" data-testid={`pip-eval-${gameId}`}>
          {formatEvaluation(centipawns)}
        </span>
      </div>

      <span
        data-testid={`pip-resize-${gameId}`}
        onPointerDown={startResize}
        aria-hidden="true"
        className="absolute bottom-0 right-0 h-4 w-4 cursor-nwse-resize touch-none bg-gradient-to-br from-transparent to-gray-500/50"
      />
    </div>
  );
}
