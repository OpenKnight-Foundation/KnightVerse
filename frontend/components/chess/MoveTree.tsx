"use client";
import React, { useEffect } from "react";

interface MoveTreeProps {
  history: any[];
  onMoveClick: (index: number) => void;
  currentMoveIndex: number;
}

const MoveTree: React.FC<MoveTreeProps> = ({
  history,
  onMoveClick,
  currentMoveIndex,
}) => {
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "ArrowLeft") {
        onMoveClick(Math.max(0, currentMoveIndex - 1));
      } else if (e.key === "ArrowRight") {
        onMoveClick(Math.min(history.length - 1, currentMoveIndex + 1));
      }
    };

    const handleWheel = (e: WheelEvent) => {
      if (e.deltaY < 0) {
        onMoveClick(Math.max(0, currentMoveIndex - 1));
      } else {
        onMoveClick(Math.min(history.length - 1, currentMoveIndex + 1));
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("wheel", handleWheel);

    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("wheel", handleWheel);
    };
  }, [history, currentMoveIndex, onMoveClick]);

  const renderMoves = (moves: any[], parentIndex: number = -1) => {
    return moves.map((move, index) => {
      const moveIndex = parentIndex === -1 ? index : parentIndex;
      return (
        <div key={index}>
          <div 
            onClick={() => onMoveClick(moveIndex)} 
            style={{ 
              fontWeight: index === currentMoveIndex ? 'bold' : 'normal', 
              cursor: 'pointer',
              display: 'inline-block',
              margin: '2px'
            }}
          >
            {move.san}
          </div>
          {move.variations && move.variations.length > 0 && (
            <div style={{ marginLeft: '20px', display: 'inline' }}>
              {move.variations.map((variation: any, i: number) => (
                <div key={i} style={{ display: 'inline' }}>
                  ( {renderMoves(variation, moveIndex)} )
                </div>
              ))}
            </div>
          )}
        </div>
      )
    });
  };

  return <div style={{ fontFamily: 'monospace' }}>{renderMoves(history)}</div>;
};

export default MoveTree;