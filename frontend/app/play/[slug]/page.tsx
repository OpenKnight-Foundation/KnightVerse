
"use client";

import React from "react";
import dynamic from "next/dynamic";
import { useSearchParams } from "next/navigation";

/**
 * Play Online Page
 *
 * Loads the chess game interface dynamically on the client to prevent
 * server-side rendering issues with browser-dependent chess functionality.
 *
 * URL parameters configure the initial game state:
 * - fen: Optional FEN string used to initialize the chessboard position.
 * - minutes: Optional time control in minutes, used to initialize both clocks.
 * - color: Optional player color; defaults to white unless set to "black".
 *
 * A loading skeleton is displayed while the game engine is being loaded.
 */

// Load the game engine exclusively on the client and display a skeleton
// while the component's JavaScript bundle is loading.
const PlayGameEngine = dynamic(
  () => import("@/components/chess/PlayGameEngine"),
  {
    ssr: false,
    loading: () => (
      <div className="min-h-screen p-4 md:p-8">
        <div className="mx-auto max-w-7xl">
          {/* Placeholder for the game header and controls. */}
          <div className="mb-4 flex items-center justify-between">
            <div className="h-5 w-32 animate-pulse rounded bg-gray-700/50" />
            <div className="h-8 w-48 animate-pulse rounded-lg bg-gray-700/50" />
          </div>

          <div className="flex flex-col items-start justify-center gap-6 lg:flex-row">
            {/* Chessboard loading placeholder. */}
            <div className="w-full min-w-0 max-w-[600px] px-2 sm:px-0">
              {/* Placeholder for player information and clock. */}
              <div className="mb-3 flex items-center justify-between px-1">
                <div className="flex items-center gap-3">
                  <div className="h-8 w-8 animate-pulse rounded-full bg-gradient-to-br from-red-400 to-red-600" />

                  <div className="space-y-1">
                    <div className="h-4 w-20 animate-pulse rounded bg-gray-700/50" />
                    <div className="h-3 w-12 animate-pulse rounded bg-gray-700/50" />
                  </div>
                </div>

                <div className="flex items-center gap-2 rounded-lg border border-gray-700/30 bg-gray-800/60 px-3 py-1.5">
                  <div className="h-4 w-12 animate-pulse rounded bg-gray-700/50" />
                </div>
              </div>

              {/* Render 64 placeholder squares to match the chessboard layout. */}
              <div className="aspect-square w-full min-w-[320px] max-w-[560px] rounded-md border-2 border-gray-700/50 p-1">
                <div className="grid h-full w-full grid-cols-8 grid-rows-8 gap-0">
                  {Array.from({ length: 64 }).map((_, index) => (
                    <div
                      key={index}
                      className={`rounded-sm shimmer-bg ${
                        (Math.floor(index / 8) + (index % 8)) % 2 === 0
                          ? "bg-gray-700/30"
                          : "bg-gray-600/20"
                      }`}
                    />
                  ))}
                </div>
              </div>
            </div>

            {/* Placeholders for game information, move history, and controls. */}
            <div className="w-full space-y-4 lg:w-80">
              <div className="h-32 animate-pulse rounded-xl border border-gray-700/50 bg-gray-800/40" />
              <div className="h-48 animate-pulse rounded-xl border border-gray-700/50 bg-gray-800/40" />
              <div className="h-40 animate-pulse rounded-xl border border-gray-700/50 bg-gray-800/40" />
            </div>
          </div>
        </div>
      </div>
    ),
  },
);

/**
 * Renders the online chess game using configuration supplied through
 * URL search parameters.
 *
 * Invalid or missing time controls fall back to the game engine's default.
 * The player color defaults to white unless the URL explicitly specifies black.
 */
export default function PlayOnlinePage() {
  const searchParams = useSearchParams();

  // Read the optional starting position from the URL.
  const fen = searchParams.get("fen") ?? undefined;

  // FE-116: Parse the selected time control and use it when it is positive
  // and finite. Otherwise, let the game engine apply its default time control.
  const minutes = Number(searchParams.get("minutes"));

  const initialMinutes =
    Number.isFinite(minutes) && minutes > 0 ? minutes : undefined;

  // FE-115: Apply the assigned player color from matchmaking.
  // Default to white when the color parameter is missing or invalid.
  const playerColor =
    searchParams.get("color") === "black" ? "black" : "white";

  return (
    <PlayGameEngine
      fen={fen}
      initialMinutes={initialMinutes}
      playerColor={playerColor}
    />
  );
}
