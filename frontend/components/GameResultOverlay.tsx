"use client";

import React from "react";
import { FaCrown, FaRobot, FaHandshake, FaRedoAlt, FaUsers } from "react-icons/fa";

type GameResult = "white_wins" | "black_wins" | "draw" | "stalemate";

interface GameResultOverlayProps {
  result: GameResult;
  onPlayAgain: () => void;
  onPlayOnline: () => void;
}

const resultConfig: Record<
  GameResult,
  { icon: React.ReactNode; title: string; subtitle: string; gradient: string; borderColor: string }
> = {
  white_wins: {
    icon: <FaCrown className="text-5xl text-yellow-400 drop-shadow-lg" />,
    title: "You Win!",
    subtitle: "Checkmate — Excellent play! ♔",
    gradient: "from-yellow-500/20 to-amber-600/10",
    borderColor: "border-yellow-500/30",
  },
  black_wins: {
    icon: <FaRobot className="text-5xl text-red-400 drop-shadow-lg" />,
    title: "Bot Wins",
    subtitle: "Checkmate — Better luck next time! ♚",
    gradient: "from-red-500/20 to-rose-600/10",
    borderColor: "border-red-500/30",
  },
  draw: {
    icon: <FaHandshake className="text-5xl text-blue-400 drop-shadow-lg" />,
    title: "Draw",
    subtitle: "Well fought — nobody wins this time",
    gradient: "from-blue-500/20 to-indigo-600/10",
    borderColor: "border-blue-500/30",
  },
  stalemate: {
    icon: <FaHandshake className="text-5xl text-gray-400 drop-shadow-lg" />,
    title: "Stalemate",
    subtitle: "No legal moves — game drawn",
    gradient: "from-gray-500/20 to-slate-600/10",
    borderColor: "border-gray-500/30",
  },
};

export function GameResultOverlay({ result, onPlayAgain, onPlayOnline }: GameResultOverlayProps) {
  const cfg = resultConfig[result];

  return (
    <div className="fixed inset-0 bg-black/70 backdrop-blur-md z-50 flex items-center justify-center animate-overlay-in">
      <div
        className={`bg-gradient-to-b ${cfg.gradient} bg-gray-900/95 p-8 rounded-3xl border ${cfg.borderColor} text-center animate-modal-in max-w-sm w-full mx-4 shadow-2xl`}
      >
        <div className="flex flex-col items-center gap-5">
          {/* Icon */}
          <div className="animate-scale-in">{cfg.icon}</div>

          {/* Title */}
          <div>
            <h2 className="text-3xl font-bold text-white tracking-tight">{cfg.title}</h2>
            <p className="text-gray-400 text-sm mt-1.5">{cfg.subtitle}</p>
          </div>

          {/* Buttons */}
          <div className="flex flex-col gap-3 w-full mt-2">
            <button
              onClick={onPlayAgain}
              className="w-full flex items-center justify-center gap-2 px-5 py-3 bg-gradient-to-r from-teal-500 to-blue-600 hover:from-teal-400 hover:to-blue-500 text-white font-semibold rounded-xl shadow-lg shadow-teal-500/20 hover:shadow-teal-400/30 transition-all duration-300 hover:scale-[1.02] active:scale-[0.98]"
            >
              <FaRedoAlt />
              Play Again
            </button>
            <button
              onClick={onPlayOnline}
              className="w-full flex items-center justify-center gap-2 px-5 py-3 bg-gray-800/60 hover:bg-gray-700/60 border border-gray-600/40 hover:border-gray-500/50 text-white font-medium rounded-xl transition-all duration-300 hover:scale-[1.02] active:scale-[0.98]"
            >
              <FaUsers />
              Play Online
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

export type { GameResult };
export default GameResultOverlay;
