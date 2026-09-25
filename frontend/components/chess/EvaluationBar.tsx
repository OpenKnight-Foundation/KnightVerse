"use client";
import React from "react";

interface EvaluationBarProps {
  evaluation: number | null;
  mate?: number | null;
  isFlipped?: boolean;
}

export function EvaluationBar({
  evaluation,
  mate = null,
  isFlipped = false,
}: EvaluationBarProps) {
  const score = evaluation ?? 0;

  const winningProbability = 1 / (1 + Math.pow(10, -score / 400));
  let whitePercentage = winningProbability * 100;

  if (mate !== null) {
    whitePercentage = score > 0 ? 100 : 0;
  }

  const displayScore = mate ? `#M${Math.abs(mate)}` : (score / 100).toFixed(2);
  const barHeight = mate ? 100 : whitePercentage;

  return (
    <div
      className={`w-10 h-full min-h-[400px] max-h-[640px] bg-gray-800 rounded-lg overflow-hidden flex shadow-lg relative ${
        isFlipped ? "flex-col-reverse" : "flex-col"
      }`}
    >
      <div
        className="w-full bg-white transition-all duration-300 ease-out"
        style={{ height: `${barHeight}%` }}
      />
      <div
        className="w-full bg-gray-900 transition-all duration-300 ease-out"
        style={{ height: `${100 - barHeight}%` }}
      />

      <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
        <span
          className={`text-sm font-bold px-2 py-1 rounded ${
            whitePercentage > 50 ? "text-black" : "text-white"
          }`}
        >
          {displayScore}
        </span>
      </div>
    </div>
  );
}
