import React, { memo, useCallback } from 'react';
import { useFeatureFlag } from "@/context/featureFlagContext";

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

/**
 * OptimizedChessboard — new board renderer.
 *
 * Gated behind the `new_board_renderer` feature flag.
 * Configure rollout via NEXT_PUBLIC_FEATURE_FLAGS in your .env.local:
 *
 *   '{"new_board_renderer":{"rollout":0.2}}'  → 20 % of users
 *   '{"new_board_renderer":{"enabled":true}}' → everyone
 */
export const OptimizedChessboard = memo(({ onMove }: { onMove: (square: number) => void }) => {
  const { isEnabled } = useFeatureFlag();
  const handleMove = useCallback((square: number) => {
    onMove(square);
  }, [onMove]);

  if (!isEnabled("new_board_renderer")) {
    // Flag is off — render nothing so the caller falls back to ChessboardComponent.
    return null;
  }

  return (
    <div className="chessboard">
      {[...Array(64)].map((_, i) => (
        <ChessSquare key={i} square={i} onMove={handleMove} />
      ))}
    </div>
  );
});
OptimizedChessboard.displayName = 'OptimizedChessboard';
