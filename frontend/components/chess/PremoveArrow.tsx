"use client";
import React from "react";

interface PremoveArrowProps {
  from: string;
  to: string;
  color: string;
}

const squareToPercent = (sq: string) => {
  if (!sq || sq.length < 2) return { x: 0, y: 0 };
  const col = sq.charCodeAt(0) - 97;
  const row = 8 - parseInt(sq[1], 10);
  return {
    x: (col + 0.5) * 12.5,
    y: (row + 0.5) * 12.5,
  };
};

const PremoveArrow: React.FC<PremoveArrowProps> = ({ from, to, color }) => {
  const fromPos = squareToPercent(from);
  const toPos = squareToPercent(to);

  return (
    <svg
      style={{
        position: "absolute",
        top: 0,
        left: 0,
        width: "100%",
        height: "100%",
        pointerEvents: "none",
      }}
    >
      <defs>
        <marker
          id="arrowhead"
          markerWidth="10"
          markerHeight="7"
          refX="0"
          refY="3.5"
          orient="auto"
        >
          <polygon points="0 0, 10 3.5, 0 7" fill={color} />
        </marker>
      </defs>
      <line
        x1={fromPos.x}
        y1={fromPos.y}
        x2={toPos.x}
        y2={toPos.y}
        stroke={color}
        strokeWidth="5"
        markerEnd="url(#arrowhead)"
      />
    </svg>
  );
};

export default PremoveArrow;
