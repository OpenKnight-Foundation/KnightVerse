"use client";
import React from "react";

interface PremoveArrowProps {
  from: string;
  to: string;
  color: string;
}

const PremoveArrow: React.FC<PremoveArrowProps> = ({ from, to, color }) => {
  const fromPos = { x: 0, y: 0 };
  const toPos = { x: 0, y: 0 };

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
