import React, { memo, useCallback } from 'react';

const ChessPiece = memo(({ square, onClick }: { square: number; onClick: (square: number) => void }) => (
  <div onClick={() => onClick(square)}>♔</div>
));
ChessPiece.displayName = 'ChessPiece';

const ChessSquare = memo(({ square, onMove }: { square: number; onMove: (square: number) => void }) => (
  <div className="square">
    <ChessPiece square={square} onClick={onMove} />
  </div>
));
ChessSquare.displayName = 'ChessSquare';

export const OptimizedChessboard = memo(({ onMove }: { onMove: (square: number) => void }) => {
  const handleMove = useCallback((square: number) => {
    onMove(square);
  }, [onMove]);

  return (
    <div className="chessboard">
      {[...Array(64)].map((_, i) => (
        <ChessSquare key={i} square={i} onMove={handleMove} />
      ))}
    </div>
  );
});
OptimizedChessboard.displayName = 'OptimizedChessboard';
