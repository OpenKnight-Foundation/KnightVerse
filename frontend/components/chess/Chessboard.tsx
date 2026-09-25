import React, { useState, useMemo } from "react";
import { Chess } from "chess.js";
import ChessboardComponent from "./ChessboardComponent";

interface ChessboardProps {
  position: string;
  onMove: (move: any) => void;
}

const Chessboard: React.FC<ChessboardProps> = ({ position, onMove }) => {
  const game = useMemo(() => new Chess(position), [position]);
  const [selectedSquare, setSelectedSquare] = useState<string | null>(null);

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
    } catch (error) {
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
      // @ts-ignore
      onSquareClick={(square: string) => setSelectedSquare(square)}
      legalMoves={legalMoves}
    />
  );
};

export default Chessboard;
