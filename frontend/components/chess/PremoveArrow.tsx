"use client";
import React from "react";

interface PremoveArrowProps {
  from: string;
  to: string;
  color: string;
  id?: string;
  orientation?: "white" | "black";
  dashed?: boolean;
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

const PremoveArrow: React.FC<PremoveArrowProps> = ({
  from,
  to,
  color,
  id = "arrowhead",
  orientation = "white",
  dashed = false,
}) => {
  const fromPos = squareToPercent(from);
  const toPos = squareToPercent(to);
  const point = (pos: { x: number; y: number }) =>
    orientation === "black"
      ? { x: 100 - pos.x, y: 100 - pos.y }
      : pos;
  const start = point(fromPos);
  const end = point(toPos);
  const markerId = `arrowhead-${id}`;

  return (
    <svg
      viewBox="0 0 100 100"
      preserveAspectRatio="none"
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
          id={markerId}
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
        x1={start.x}
        y1={start.y}
        x2={end.x}
        y2={end.y}
        stroke={color}
        strokeWidth="5"
        strokeDasharray={dashed ? "6 4" : undefined}
        markerEnd={`url(#${markerId})`}
      />
    </svg>
  );
};

export default PremoveArrow;
