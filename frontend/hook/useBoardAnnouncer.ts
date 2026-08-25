"use client";

import { useRef, useCallback } from "react";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface MoveAnnouncementOptions {
  /** Standard algebraic notation of the move, e.g. "e4", "Nf3", "O-O" */
  san: string;
  /** Which side just moved */
  color: "w" | "b";
  /** Source square in algebraic notation (e.g. "e2") */
  from: string;
  /** Destination square in algebraic notation (e.g. "e4") */
  to: string;
  /** Piece type code (p|n|b|r|q|k) */
  piece: string;
  /** True when the move delivers check */
  isCheck?: boolean;
  /** True when the move is checkmate */
  isCheckmate?: boolean;
  /** True when the move is stalemate */
  isStalemate?: boolean;
  /** Captured piece type code, if any (p|n|b|r|q|k) */
  captured?: string;
  /** Promotion piece type code, if any */
  promotion?: string;
  /** True when the move is a castling kingside */
  isKingsideCastle?: boolean;
  /** True when the move is a castling queenside */
  isQueensideCastle?: boolean;
  /** True when the move is an en-passant capture */
  isEnPassant?: boolean;
}

export interface TimerAlertOptions {
  /** Color of the player whose clock is low */
  color: "w" | "b";
  /** Remaining seconds */
  seconds: number;
}

export interface UseBoardAnnouncerReturn {
  /** Announce a chess move with a natural-language description */
  announceMove: (opts: MoveAnnouncementOptions) => void;
  /** Announce a low-time alert */
  announceTimerAlert: (opts: TimerAlertOptions) => void;
  /** Announce an arbitrary message, e.g. game start/end */
  announceMessage: (message: string) => void;
  /** Politely announce something non-urgent */
  announcePolitely: (message: string) => void;
  /**
   * Ref to attach to the assertive aria-live region element.
   * Mount `<div ref={assertiveRef} aria-live="assertive" aria-atomic="true" className="sr-only" />`
   * near the top of your render tree.
   */
  assertiveRef: React.RefObject<HTMLDivElement | null>;
  /**
   * Ref to attach to the polite aria-live region element.
   * Mount `<div ref={politeRef} aria-live="polite" aria-atomic="true" className="sr-only" />`
   * near the top of your render tree.
   */
  politeRef: React.RefObject<HTMLDivElement | null>;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const PIECE_NAMES: Record<string, string> = {
  p: "Pawn",
  n: "Knight",
  b: "Bishop",
  r: "Rook",
  q: "Queen",
  k: "King",
};

function pieceName(code: string): string {
  return PIECE_NAMES[code.toLowerCase()] ?? code.toUpperCase();
}

function colorName(color: "w" | "b"): string {
  return color === "w" ? "White" : "Black";
}

function buildMoveDescription(opts: MoveAnnouncementOptions): string {
  const mover = colorName(opts.color);
  const piece = pieceName(opts.piece);

  // ── Castling ──
  if (opts.isKingsideCastle) {
    let msg = `${mover} castles kingside`;
    if (opts.isCheck) msg += ", check";
    if (opts.isCheckmate) msg += ", checkmate";
    return msg;
  }
  if (opts.isQueensideCastle) {
    let msg = `${mover} castles queenside`;
    if (opts.isCheck) msg += ", check";
    if (opts.isCheckmate) msg += ", checkmate";
    return msg;
  }

  // ── Normal / capture / promotion ──
  let msg = `${mover} ${piece} from ${opts.from} to ${opts.to}`;

  if (opts.isEnPassant) {
    msg += `, captures ${colorName(opts.color === "w" ? "b" : "w")} Pawn en passant`;
  } else if (opts.captured) {
    msg += `, captures ${colorName(opts.color === "w" ? "b" : "w")} ${pieceName(opts.captured)}`;
  }

  if (opts.promotion) {
    msg += `, promotes to ${pieceName(opts.promotion)}`;
  }

  if (opts.isCheckmate) {
    msg += ". Checkmate!";
  } else if (opts.isStalemate) {
    msg += ". Stalemate. Game is drawn.";
  } else if (opts.isCheck) {
    msg += `, checking ${colorName(opts.color === "w" ? "b" : "w")} King`;
  }

  return msg;
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

/**
 * useBoardAnnouncer
 *
 * Provides accessible aria-live announcements for chess moves, captures,
 * checks, and time alerts.  Exposes two ref objects that consumers must attach
 * to sr-only <div> elements in the component tree:
 *
 * ```tsx
 * const { assertiveRef, politeRef, announceMove } = useBoardAnnouncer();
 *
 * // In JSX:
 * <div ref={assertiveRef} aria-live="assertive" aria-atomic="true" className="sr-only" />
 * <div ref={politeRef}    aria-live="polite"    aria-atomic="true" className="sr-only" />
 * ```
 *
 * The hook uses a clear → set pattern with a brief delay so that identical
 * consecutive messages are still re-announced by screen readers.
 */
export function useBoardAnnouncer(): UseBoardAnnouncerReturn {
  const assertiveRef = useRef<HTMLDivElement | null>(null);
  const politeRef = useRef<HTMLDivElement | null>(null);
  const assertiveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const politeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  /** Push text into the assertive region (clears first to force re-read). */
  const speakAssertive = useCallback((message: string) => {
    const el = assertiveRef.current;
    if (!el) return;

    // Cancel any pending update so we don't clobber the incoming message.
    if (assertiveTimerRef.current !== null) {
      clearTimeout(assertiveTimerRef.current);
    }

    // Clear, then set — this forces screen readers to re-announce even if the
    // text is identical to the previous announcement.
    el.textContent = "";
    assertiveTimerRef.current = setTimeout(() => {
      el.textContent = message;
      assertiveTimerRef.current = null;
    }, 50);
  }, []);

  /** Push text into the polite region. */
  const speakPolite = useCallback((message: string) => {
    const el = politeRef.current;
    if (!el) return;

    if (politeTimerRef.current !== null) {
      clearTimeout(politeTimerRef.current);
    }

    el.textContent = "";
    politeTimerRef.current = setTimeout(() => {
      el.textContent = message;
      politeTimerRef.current = null;
    }, 50);
  }, []);

  const announceMove = useCallback(
    (opts: MoveAnnouncementOptions) => {
      const description = buildMoveDescription(opts);
      // Check / checkmate / stalemate are urgent → assertive.
      // Regular moves are polite so they don't interrupt the user.
      if (opts.isCheckmate || opts.isStalemate) {
        speakAssertive(description);
      } else if (opts.isCheck) {
        speakAssertive(description);
      } else {
        speakPolite(description);
      }
    },
    [speakAssertive, speakPolite],
  );

  const announceTimerAlert = useCallback(
    (opts: TimerAlertOptions) => {
      const player = colorName(opts.color);
      const msg =
        opts.seconds <= 10
          ? `${player} has ${opts.seconds} seconds remaining!`
          : `${player} has ${opts.seconds} seconds remaining.`;
      speakAssertive(msg);
    },
    [speakAssertive],
  );

  const announceMessage = useCallback(
    (message: string) => {
      speakAssertive(message);
    },
    [speakAssertive],
  );

  const announcePolitely = useCallback(
    (message: string) => {
      speakPolite(message);
    },
    [speakPolite],
  );

  return {
    announceMove,
    announceTimerAlert,
    announceMessage,
    announcePolitely,
    assertiveRef,
    politeRef,
  };
}
