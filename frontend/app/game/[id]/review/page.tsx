
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
 * Game Review Page (FE-120, Issue #1361)
 *
 * Provides a post-game review interface by combining the PGN replay viewer,
 * centipawn advantage chart, review summary, and PGN export/archive tools.
 *
 * The game's PGN is retrieved from sessionStorage, where PlayGameEngine
 * stores it when a game ends. If the stored PGN is missing or inaccessible,
 * the page falls back to a mock game for demonstration purposes.
 *
 * Engine evaluation data is not yet integrated, so move evaluations and
 * player accuracy statistics currently use neutral placeholder values.
 *
 * Features:
 * - Replays completed games from their PGN notation.
 * - Parses move history for the evaluation chart.
 * - Tracks the currently selected move in the review interface.
 * - Displays a summary of player accuracy and move classifications.
 * - Provides PGN export and archive functionality.
 */

// Prefix used to associate stored PGN data with a specific game.
const REVIEW_STORAGE_PREFIX = "knightverse:review:";

// Default move-classification counts used until game analysis is available.
const ZERO_COUNTS: Record<MoveClass, number> = {
  brilliant: 0,
  great: 0,
  best: 0,
  inaccuracy: 0,
  mistake: 0,
  blunder: 0,
};

/**
 * Renders the review interface for a completed chess game.
 *
 * Reads the game ID from the route, loads the corresponding PGN from
 * sessionStorage, parses its move history, and passes the data to the
 * relevant review components.
 */
export default function GameReviewPage() {
  const params = useParams<{ id: string }>();
  const gameId = params?.id ?? "unknown";

  const [pgn, setPgn] = useState<string>("");
  const [activePly, setActivePly] = useState(0);

  // Load the saved PGN when the game ID changes.
  // Fall back to the mock game if storage is unavailable or no PGN is saved.
  useEffect(() => {
    if (!gameId || gameId === "unknown") {
      setPgn(MOCK_PGN);
      return;
    }

    try {
      const storedPgn = sessionStorage.getItem(
        `${REVIEW_STORAGE_PREFIX}${gameId}`,
      );

      setPgn(
        storedPgn && storedPgn.trim().length > 0 ? storedPgn : MOCK_PGN,
      );
    } catch {
      // Handle restricted storage access without preventing the review page
      // from rendering.
      setPgn(MOCK_PGN);
    }
  }, [gameId]);

  // Parse the PGN into individual moves for the evaluation chart.
  // Evaluations remain null until chess-engine analysis is integrated.
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
      // Return an empty move list when the PGN is invalid or cannot be parsed.
      return [];
    }
  }, [pgn]);

  return (
    <main
      className="min-h-screen p-4 text-white md:p-8"
      aria-label="Game review"
    >
      <div className="mx-auto max-w-5xl space-y-6">
        {/* Page heading and navigation back to the game lobby. */}
        <header className="flex items-center justify-between gap-4">
          <div>
            <h1 className="bg-gradient-to-r from-teal-400 to-blue-500 bg-clip-text text-3xl font-extrabold text-transparent">
              Game Review
            </h1>

            <p className="mt-1 text-sm text-gray-400">
              Replay, analyse and export game {gameId.slice(0, 12)}
            </p>
          </div>

          <Link
            href="/"
            className="text-sm text-gray-400 transition-colors hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-teal-400"
          >
            Back to Lobby
          </Link>
        </header>

        {/* Interactive replay of the selected game's PGN. */}
        <section
          className="rounded-2xl border border-gray-700/50 bg-gray-900/60 p-4"
          aria-label="Game replay"
        >
          <GameHistoryPGNViewer pgn={pgn || MOCK_PGN} />
        </section>

        {/* Move-by-move evaluation visualization. */}
        <section
          className="rounded-2xl border border-gray-700/50 bg-gray-900/60 p-4"
          aria-labelledby="evaluation-heading"
        >
          <h2
            id="evaluation-heading"
            className="mb-3 text-sm font-semibold text-gray-300"
          >
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
        </section>

        {/* Player accuracy and move-classification summary. */}
        <section aria-label="Game review summary">
          <GameReviewSummary
            whiteAccuracy={0}
            blackAccuracy={0}
            counts={ZERO_COUNTS}
          />
        </section>

        {/* Export or archive the PGN associated with the current game. */}
        <section
          className="rounded-2xl border border-gray-700/50 bg-gray-900/60 p-4"
          aria-label="PGN export and archive"
        >
          <PgnExportArchive
            gameId={gameId}
            pgnContent={pgn || MOCK_PGN}
          />
        </section>
      </div>
    </main>
  );
}
