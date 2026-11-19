"use client";

import { useState, useCallback, useEffect, useRef } from "react";

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
  /** The current move announcement text to render in a polite aria-live region */
  announcement: string;
  /** The current time alert to render in an assertive aria-live region */
  timeAnnouncement: string;
  /** Announce a move that was just played */
  announceMove: (move: MoveAnnouncement) => void;
  /** Announce a time alert (e.g. "Less than 30 seconds remaining") */
  announceTimeAlert: (message: string) => void;
}

function createPusher(
  setValue: (value: string) => void,
  timeoutRef: { current: ReturnType<typeof setTimeout> | null },
) {
  return (message: string) => {
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
    }
    setValue("");
    timeoutRef.current = setTimeout(() => {
      setValue(message);
      timeoutRef.current = null;
    }, 50);
  };
}

export function useBoardAnnouncer(): UseBoardAnnouncerReturn {
  const [announcement, setAnnouncement] = useState("");
  const [timeAnnouncement, setTimeAnnouncement] = useState("");
  const moveTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const timeTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const pushMove = useCallback(createPusher(setAnnouncement, moveTimeoutRef), []);
  const pushTime = useCallback(createPusher(setTimeAnnouncement, timeTimeoutRef), []);

  useEffect(() => {
    return () => {
      if (moveTimeoutRef.current) clearTimeout(moveTimeoutRef.current);
      if (timeTimeoutRef.current) clearTimeout(timeTimeoutRef.current);
    };
  }, []);

  const announceMove = useCallback(
    (move: MoveAnnouncement) => {
      pushMove(describeMove(move));
    },
    [pushMove],
  );

  const announceTimeAlert = useCallback(
    (message: string) => {
      pushTime(`Time alert: ${message}`);
    },
    [pushTime],
  );

  return {
    announcement,
    timeAnnouncement,
    announceMove,
    announceTimeAlert,
  };
}
