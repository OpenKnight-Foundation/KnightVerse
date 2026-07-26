import React, { memo, useCallback } from 'react';

const ChessPiece = memo(({ square, onClick }) => (
  <div onClick={() => onClick(square)}>♔</div>
));

const ChessSquare = memo(({ square, onMove }) => (
  <div className="square">
    <ChessPiece square={square} onClick={onMove} />
  </div>
));

export const OptimizedChessboard = memo(({ boardState, onMove }) => {
  const handleMove = useCallback((square) => {
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
