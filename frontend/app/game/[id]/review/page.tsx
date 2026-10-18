"use client";

import React, { useEffect, useMemo, useState } from "react";
import Link from "next/link";
import { useParams } from "next/navigation";
import { Chess } from "chess.js";
import {
  GameHistoryPGNViewer,
  MOCK_PGN,
} from "@/components/chess/GameHistoryPGNViewer";
import { PgnExportArchive } from "@/components/chess/PgnExportArchive";
import GameReviewSummary, {
  type MoveClass,
} from "@/components/chess/GameReviewSummary";
import CentipawnAdvantageChart, {
  type EvaluatedPly,
} from "@/components/chess/CentipawnAdvantageChart";

/**
 * FE-120 (#1361): post-game review view. It composes the PGN replay viewer,
 * centipawn chart, review summary and PGN export/archive components that were
 * previously built but never rendered.
 *
 * The game's PGN is read from sessionStorage (written by PlayGameEngine when the
 * game ends); engine evaluation data is not available yet, so the chart and
 * accuracy summary render with neutral placeholders until analysis is wired in.
 */
const REVIEW_STORAGE_PREFIX = "knightverse:review:";

const ZERO_COUNTS: Record<MoveClass, number> = {
  brilliant: 0,
  great: 0,
  best: 0,
  inaccuracy: 0,
  mistake: 0,
  blunder: 0,
};

export default function GameReviewPage() {
  const params = useParams<{ id: string }>();
  const gameId = params?.id ?? "unknown";
  const [pgn, setPgn] = useState<string>("");
  const [activePly, setActivePly] = useState(0);

  useEffect(() => {
    if (!gameId) return;
    try {
      const stored = sessionStorage.getItem(`${REVIEW_STORAGE_PREFIX}${gameId}`);
      setPgn(stored && stored.trim().length > 0 ? stored : MOCK_PGN);
    } catch {
      setPgn(MOCK_PGN);
    }
  }, [gameId]);

  const plies: EvaluatedPly[] = useMemo(() => {
    if (!pgn) return [];
    try {
      const game = new Chess();
      game.loadPgn(pgn);
      return game.history().map((san, index) => ({
        plyIndex: index,
        evaluation: null,
        san,
      }));
    } catch {
      return [];
    }
  }, [pgn]);

  return (
    <div
      className="min-h-screen p-4 md:p-8 text-white"
      role="main"
      aria-label="Game review"
    >
      <div className="max-w-5xl mx-auto space-y-6">
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-3xl font-extrabold bg-gradient-to-r from-teal-400 to-blue-500 bg-clip-text text-transparent">
              Game Review
            </h1>
            <p className="text-gray-400 text-sm mt-1">
              Replay, analyse and export game {gameId.slice(0, 12)}
            </p>
          </div>
          <Link
            href="/"
            className="text-sm text-gray-400 hover:text-white transition-colors"
          >
            Back to Lobby
          </Link>
        </div>

        <div className="rounded-2xl border border-gray-700/50 bg-gray-900/60 p-4">
          <GameHistoryPGNViewer pgn={pgn || MOCK_PGN} />
        </div>

        <div className="rounded-2xl border border-gray-700/50 bg-gray-900/60 p-4">
          <h2 className="text-sm font-semibold text-gray-300 mb-3">
            Evaluation
          </h2>
          {plies.length > 0 ? (
            <CentipawnAdvantageChart
              plies={plies}
              activePly={activePly}
              onPlySelect={setActivePly}
            />
          ) : (
            <p className="text-sm text-gray-500">
              Evaluation data is not available for this game yet.
            </p>
          )}
        </div>

        <GameReviewSummary
          whiteAccuracy={0}
          blackAccuracy={0}
          counts={ZERO_COUNTS}
        />

        <div className="rounded-2xl border border-gray-700/50 bg-gray-900/60 p-4">
          <PgnExportArchive gameId={gameId} pgnContent={pgn || MOCK_PGN} />
        </div>
      </div>
    </div>
  );
}
