"use client";

import React from "react";

export type MoveClass =
  | "brilliant"
  | "great"
  | "best"
  | "inaccuracy"
  | "mistake"
  | "blunder";

interface GameReviewSummaryProps {
  whiteAccuracy: number;
  blackAccuracy: number;
  counts: Record<MoveClass, number>;
  onRetryMistake?: () => void;
}

const BADGE: Record<MoveClass, string> = {
  brilliant: "!!",
  great: "!",
  best: "★",
  inaccuracy: "?!",
  mistake: "?",
  blunder: "??",
};

/**
 * FE-50: game review summary card. Accuracy % and per-move classification
 * are computed upstream (engine analysis) and passed in as props.
 */
const GameReviewSummary: React.FC<GameReviewSummaryProps> = ({
  whiteAccuracy,
  blackAccuracy,
  counts,
  onRetryMistake,
}) => {
  return (
    <div className="game-review-summary p-4 rounded-lg border border-white/10">
      <div className="flex justify-between text-sm mb-3">
        <span>White accuracy: {whiteAccuracy.toFixed(1)}%</span>
        <span>Black accuracy: {blackAccuracy.toFixed(1)}%</span>
      </div>
      <ul className="flex flex-wrap gap-3 text-sm">
        {(Object.keys(BADGE) as MoveClass[]).map((cls) => (
          <li key={cls}>
            {BADGE[cls]} {cls}: {counts[cls] ?? 0}
          </li>
        ))}
      </ul>
      {counts.blunder > 0 && onRetryMistake && (
        <button onClick={onRetryMistake} className="mt-3 px-3 py-1.5 rounded bg-red-600 text-white text-sm">
          Retry Mistake
        </button>
      )}
    </div>
  );
};

export default GameReviewSummary;
