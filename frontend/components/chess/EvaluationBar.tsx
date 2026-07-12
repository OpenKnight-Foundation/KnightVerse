"use client";

import React from "react";

interface EvaluationBarProps {
  evaluation: number | null; // Pawn advantage, positive for white, negative for black
}

export function EvaluationBar({ evaluation }: EvaluationBarProps) {
  const score = evaluation ?? 0;
  
  // We'll map the score so that -10 pawns = 100% black height, +10 pawns = 0% black height
  const clamp = (val: number, min: number, max: number) => Math.min(Math.max(val, min), max);
  const blackPercentage = clamp(50 - (score / 10) * 50, 0, 100);
  const displayScore = score > 0 ? `+${score.toFixed(1)}` : score.toFixed(1);

  return (
    <div className="w-8 h-full min-h-[320px] max-h-[560px] bg-white rounded-md overflow-hidden flex flex-col border-2 border-gray-700 shadow-lg relative shrink-0">
      <div 
        className="w-full bg-gray-900 transition-all duration-700 ease-out"
        style={{ height: `${blackPercentage}%` }}
      />
      
      {/* Score Text inside the bar */}
      <div className="absolute inset-0 flex items-center justify-center pointer-events-none opacity-60 hover:opacity-100 transition-opacity">
        <span className={`text-xs font-bold px-1 py-0.5 rounded bg-black/40 backdrop-blur-sm ${blackPercentage < 50 ? 'text-gray-900 bg-white/40' : 'text-white'}`}>
          {displayScore}
        </span>
      </div>
    </div>
  );
}
