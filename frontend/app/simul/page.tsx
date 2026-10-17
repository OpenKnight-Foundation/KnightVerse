"use client";

import React from "react";
import SimulHostDashboard, { type SimulGame } from "@/components/chess/SimulHostDashboard";

/**
 * FE-122 (#1363): mount the simultaneous-exhibition host dashboard at a real
 * route so hosts can run many boards at once.
 *
 * Sample boards are shown until the backend exposes a live simul session
 * feed; wiring that feed is tracked as a separate backend dependency.
 */
const SAMPLE_GAMES: SimulGame[] = [
  {
    id: "simul-1",
    opponentName: "PawnStorm",
    opponentElo: 1420,
    fen: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1",
    status: "opponent_turn",
    hostTimeRemaining: 1800,
    opponentTimeRemaining: 1800,
    moveCount: 1,
    lastMove: "e4",
    boardIndex: 0,
  },
  {
    id: "simul-2",
    opponentName: "RookieRook",
    opponentElo: 1310,
    fen: "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2",
    status: "host_turn",
    hostTimeRemaining: 1795,
    opponentTimeRemaining: 1797,
    moveCount: 2,
    lastMove: "e5",
    boardIndex: 1,
  },
  {
    id: "simul-3",
    opponentName: "QuietQueen",
    opponentElo: 1650,
    fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
    status: "waiting",
    hostTimeRemaining: 1800,
    opponentTimeRemaining: 1800,
    moveCount: 0,
    boardIndex: 2,
  },
];

export default function SimulPage() {
  return (
    <div
      className="min-h-screen p-4 md:p-8 text-white"
      role="main"
      aria-label="Simultaneous exhibition"
    >
      <div className="max-w-7xl mx-auto">
        <h1 className="text-3xl font-extrabold bg-gradient-to-r from-teal-400 to-blue-500 bg-clip-text text-transparent">
          Simultaneous Exhibition
        </h1>
        <p className="text-gray-400 text-sm mt-1 mb-6">
          Play many opponents at once and manage every board from one dashboard.
        </p>
        <SimulHostDashboard initialGames={SAMPLE_GAMES} hostName="You" />
      </div>
    </div>
  );
}
