"use client";

import { useState, useCallback, useRef } from "react";

export interface MoveAnnouncement {
  /** "w" for white, "b" for black */
  color: "w" | "b";
  /** Whether the move is a capture */
  isCapture: boolean;
  /** Whether the move gives check */
  isCheck: boolean;
  /** The piece letter (K, Q, R, B, N, P) */
  piece: string;
  /** Source square in algebraic notation (e.g. "g1") */
  from: string;
  /** Target square in algebraic notation (e.g. "f3") */
  to: string;
}

const PIECE_NAMES: Record<string, string> = {
  K: "King",
  Q: "Queen",
  R: "Rook",
  B: "Bishop",
  N: "Knight",
  P: "Pawn",
};

function describeMove(announcement: MoveAnnouncement): string {
  const colorName = announcement.color === "w" ? "White" : "Black";
  const pieceName = PIECE_NAMES[announcement.piece] ?? "Pawn";

  let description = `${colorName} moved ${pieceName} from ${announcement.from} to ${announcement.to}`;

  if (announcement.isCapture) {
    description += ", capturing";
  }

  if (announcement.isCheck) {
    description += `, checking the ${announcement.color === "w" ? "Black" : "White"} King`;
  }

  return description;
}

export interface UseBoardAnnouncerReturn {
  /** The current announcement text to render in the aria-live region */
  announcement: string;
  /** Announce a move that was just played */
  announceMove: (move: MoveAnnouncement) => void;
  /** Announce a time alert (e.g. "Less than 30 seconds remaining") */
  announceTimeAlert: (message: string) => void;
}

export function useBoardAnnouncer(): UseBoardAnnouncerReturn {
  const [announcement, setAnnouncement] = useState("");
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const push = useCallback((msg: string) => {
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
    }
    // Clear first so re-announcement of the same text still triggers screen readers
    setAnnouncement("");
    timeoutRef.current = setTimeout(() => {
      setAnnouncement(msg);
      timeoutRef.current = null;
    }, 50);
  }, []);

  const announceMove = useCallback(
    (move: MoveAnnouncement) => {
      push(describeMove(move));
    },
    [push],
  );

  const announceTimeAlert = useCallback(
    (message: string) => {
      push(`Time alert: ${message}`);
    },
    [push],
  );

  return {
    announcement,
    announceMove,
    announceTimeAlert,
  };
}
