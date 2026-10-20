import React, { useState, useMemo } from "react";
import { Chess, type Move, type Square } from "chess.js";
import ChessboardComponent from "./ChessboardComponent";

/**
 * Thin adapter over the canonical `ChessboardComponent` (FE-123 / #1364).
 *
 * `ChessboardComponent` is the single board implementation; this wrapper only
 * adds click-to-select legal-move handling for callers that want the simpler
 * `onMove` API. The duplicate `OptimizedChessboard` renderer was removed and the
 * WebGL board is opt-in via the `boardRenderer` appearance setting.
 */
interface ChessboardProps {
  position: string;
  onMove: (move: Move) => void;
}

const Chessboard: React.FC<ChessboardProps> = ({ position, onMove }) => {
  const game = useMemo(
    () => new Chess(position === "start" ? undefined : position),
    [position],
  );
  const [selectedSquare, setSelectedSquare] = useState<Square | null>(null);

  const handleDrop = ({
    sourceSquare,
    targetSquare,
  }: {
    sourceSquare: string;
    targetSquare: string;
  }) => {
    try {
      const move = game.move({
        from: sourceSquare,
        to: targetSquare,
        promotion: "q", // always promote to a queen for simplicity
      });

      if (move) {
        onMove(move);
        return true;
      }
    } catch {
      // illegal move
    }
    return false;
  };

  const legalMoves = useMemo(() => {
    if (!selectedSquare) {
      return [];
    }
    return game
      .moves({ square: selectedSquare, verbose: true })
      .map((move) => move.to);
  }, [game, selectedSquare]);

  return (
    <ChessboardComponent
      position={game.fen()}
      onDrop={handleDrop}
      onSquareClick={(square: string) => setSelectedSquare(square as Square)}
      legalMoves={legalMoves}
    />
  );
};

export default Chessboard;
