import { Chess } from "chess.js";
import type { SpectatorMove } from "@/hook/useSpectatorSocket";

const WHITE_CAPTURED_SYMBOLS: Record<string, string> = {
  p: "♟",
  n: "♞",
  b: "♝",
  r: "♜",
  q: "♛",
  k: "♚",
};

const BLACK_CAPTURED_SYMBOLS: Record<string, string> = {
  p: "♙",
  n: "♘",
  b: "♗",
  r: "♖",
  q: "♕",
  k: "♔",
};

export function formatClock(seconds: number): string {
  const safeSeconds = Math.max(0, Math.floor(seconds));
  const minutes = Math.floor(safeSeconds / 60);
  const remainingSeconds = safeSeconds % 60;
  return `${minutes}:${remainingSeconds.toString().padStart(2, "0")}`;
}

export function truncateAddress(address: string): string {
  if (address.length <= 11) {
    return address;
  }

  return `${address.slice(0, 6)}...${address.slice(-5)}`;
}

export function getCapturedFromMoves(moves: SpectatorMove[]): {
  white: string[];
  black: string[];
} {
  const chess = new Chess();
  const captured = {
    white: [] as string[],
    black: [] as string[],
  };

  for (const move of moves) {
    try {
      const verboseMove = chess.move({
        from: move.from,
        to: move.to,
        promotion: move.promotion ?? "q",
      });

      if (!verboseMove?.captured) {
        continue;
      }

      if (move.color === "w") {
        captured.white.push(verboseMove.captured);
      } else {
        captured.black.push(verboseMove.captured);
      }
    } catch {
      continue;
    }
  }

  return captured;
}

export function getGamePhase(moveCount: number): "opening" | "middlegame" | "endgame" {
  if (moveCount <= 10) {
    return "opening";
  }

  if (moveCount <= 30) {
    return "middlegame";
  }

  return "endgame";
}

export function getCapturedPieceSymbols(
  pieces: string[],
  capturedBy: "white" | "black",
): string[] {
  const symbolMap = capturedBy === "white" ? WHITE_CAPTURED_SYMBOLS : BLACK_CAPTURED_SYMBOLS;
  return pieces.map((piece) => symbolMap[piece] ?? piece.toUpperCase());
}
