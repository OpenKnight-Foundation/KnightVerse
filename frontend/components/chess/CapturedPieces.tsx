"use client";

import React from "react";
import Image from "next/image";

import WhiteQueen from "./chesspieces/white-queen.svg";
import WhiteBishop from "./chesspieces/white-bishop.svg";
import WhiteKnight from "./chesspieces/white-knight.svg";
import WhiteRook from "./chesspieces/white-rook.svg";
import WhitePawn from "./chesspieces/white-pawn.svg";
import BlackQueen from "./chesspieces/black-queen.svg";
import BlackBishop from "./chesspieces/black-bishop.svg";
import BlackKnight from "./chesspieces/black-knight.svg";
import BlackRook from "./chesspieces/black-rook.svg";
import BlackPawn from "./chesspieces/black-pawn.svg";

const pieceImages: Record<string, string> = {
  P: WhitePawn,
  R: WhiteRook,
  N: WhiteKnight,
  B: WhiteBishop,
  Q: WhiteQueen,
  p: BlackPawn,
  r: BlackRook,
  n: BlackKnight,
  b: BlackBishop,
  q: BlackQueen,
};

// Points for simple evaluation (if we wanted to show score difference)
const pieceValues: Record<string, number> = {
  p: 1, P: 1,
  n: 3, N: 3,
  b: 3, B: 3,
  r: 5, R: 5,
  q: 9, Q: 9,
};

interface CapturedPiecesProps {
  fen: string;
  color: "white" | "black";
}

export const CapturedPieces: React.FC<CapturedPiecesProps> = ({ fen, color }) => {
  // Determine what pieces the `color` player has captured.
  // If color is "white", we look at missing black pieces.
  
  const initialCounts = {
    white: { P: 8, N: 2, B: 2, R: 2, Q: 1 },
    black: { p: 8, n: 2, b: 2, r: 2, q: 1 },
  };

  const currentCounts = {
    white: { P: 0, N: 0, B: 0, R: 0, Q: 0 },
    black: { p: 0, n: 0, b: 0, r: 0, q: 0 },
  };

  const boardPart = fen === "start" ? "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR" : fen.split(" ")[0];
  
  for (let i = 0; i < boardPart.length; i++) {
    const char = boardPart[i];
    if (char >= 'a' && char <= 'z' && currentCounts.black[char as keyof typeof currentCounts.black] !== undefined) {
      currentCounts.black[char as keyof typeof currentCounts.black]++;
    } else if (char >= 'A' && char <= 'Z' && currentCounts.white[char as keyof typeof currentCounts.white] !== undefined) {
      currentCounts.white[char as keyof typeof currentCounts.white]++;
    }
  }

  const capturedPieces: string[] = [];
  
  if (color === "white") {
    // White captures black pieces
    for (const [p, count] of Object.entries(initialCounts.black)) {
      const missing = Math.max(0, count - currentCounts.black[p as keyof typeof currentCounts.black]);
      for (let i = 0; i < missing; i++) capturedPieces.push(p);
    }
  } else {
    // Black captures white pieces
    for (const [p, count] of Object.entries(initialCounts.white)) {
      const missing = Math.max(0, count - currentCounts.white[p as keyof typeof currentCounts.white]);
      for (let i = 0; i < missing; i++) capturedPieces.push(p);
    }
  }

  // Sort by value (Queen first)
  capturedPieces.sort((a, b) => pieceValues[b] - pieceValues[a]);

  if (capturedPieces.length === 0) return <div className="h-6 w-full" />; // placeholder height

  return (
    <div className="flex flex-wrap items-center gap-1 h-6">
      {capturedPieces.map((piece, idx) => (
        <div key={`${piece}-${idx}`} className="relative w-5 h-5">
          <Image
            src={pieceImages[piece]}
            alt={piece}
            fill
            className="object-contain drop-shadow-sm opacity-90"
          />
        </div>
      ))}
    </div>
  );
};
