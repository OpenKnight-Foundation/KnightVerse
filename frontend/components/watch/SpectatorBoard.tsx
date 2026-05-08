"use client";

import { useEffect, useMemo, useState } from "react";
import dynamic from "next/dynamic";
import { Chess } from "chess.js";
import { AlertTriangle, Eye, LoaderCircle, LogOut, RefreshCcw, Signal } from "lucide-react";
import { PlayerPanel } from "@/components/watch/PlayerPanel";
import { SpectatorMoveList } from "@/components/watch/SpectatorMoveList";
import { useSpectatorSocket } from "@/hook/useSpectatorSocket";
import { getGamePhase, getCapturedFromMoves } from "@/lib/spectatorUtils";
import { cn } from "@/lib/utils";

const ChessboardComponent = dynamic(
  () => import("@/components/chess/ChessboardComponent"),
  {
    ssr: false,
    loading: () => (
      <div className="w-full max-w-[560px] min-w-[320px] aspect-square rounded-md border-2 border-gray-700/50 p-1">
        <div className="grid grid-cols-8 grid-rows-8 gap-0 w-full h-full">
          {Array.from({ length: 64 }).map((_, i) => (
            <div
              key={i}
              className={`${(Math.floor(i / 8) + (i % 8)) % 2 === 0 ? "bg-gray-700/30" : "bg-gray-600/20"} rounded-sm shimmer-bg`}
            />
          ))}
        </div>
      </div>
    ),
  },
);

interface SpectatorBoardProps {
  gameId: string;
  onLeave: () => void;
}

function getStatusClasses(status: string) {
  switch (status) {
    case "connected":
      return "border-emerald-500/30 bg-emerald-500/15 text-emerald-400";
    case "reconnecting":
    case "connecting":
      return "border-amber-500/30 bg-amber-500/15 text-amber-300";
    case "error":
      return "border-red-500/30 bg-red-500/15 text-red-400";
    default:
      return "border-gray-700/40 bg-gray-700/20 text-gray-300";
  }
}

function getGameOverText(status: string, result?: string) {
  switch (status) {
    case "checkmate":
      return {
        title: "Checkmate",
        subtitle: result ? `Result ${result}` : "The king has fallen.",
      };
    case "stalemate":
      return {
        title: "Stalemate",
        subtitle: result ? `Result ${result}` : "No legal moves remain.",
      };
    case "draw":
      return {
        title: "Draw",
        subtitle: result ? `Result ${result}` : "Players split the point.",
      };
    case "resigned":
      return {
        title: "Resignation",
        subtitle: result ? `Result ${result}` : "A player has resigned.",
      };
    default:
      return null;
  }
}

export function SpectatorBoard({ gameId, onLeave }: SpectatorBoardProps) {
  const { status, gameState, disconnect, reconnect } = useSpectatorSocket(gameId);
  const [displayClocks, setDisplayClocks] = useState({ white: 0, black: 0 });

  useEffect(() => {
    if (!gameState) {
      setDisplayClocks({ white: 0, black: 0 });
      return;
    }

    setDisplayClocks({
      white: gameState.whiteTime,
      black: gameState.blackTime,
    });
  }, [gameState?.whiteTime, gameState?.blackTime, gameState]);

  const activeColor = useMemo(() => {
    if (!gameState || gameState.status !== "playing") {
      return null;
    }

    try {
      return new Chess(gameState.fen).turn() === "w" ? "white" : "black";
    } catch {
      return null;
    }
  }, [gameState]);

  useEffect(() => {
    if (!gameState || gameState.status !== "playing" || !activeColor) {
      return;
    }

    const intervalId = window.setInterval(() => {
      setDisplayClocks((previousClocks) => {
        if (activeColor === "white") {
          return {
            white: Math.max(0, previousClocks.white - 1),
            black: previousClocks.black,
          };
        }

        return {
          white: previousClocks.white,
          black: Math.max(0, previousClocks.black - 1),
        };
      });
    }, 1000);

    return () => {
      clearInterval(intervalId);
    };
  }, [activeColor, gameState]);

  const capturedPieces = useMemo(
    () => getCapturedFromMoves(gameState?.moves ?? []),
    [gameState?.moves],
  );

  const gameOverCopy = getGameOverText(gameState?.status ?? "playing", gameState?.result);
  const moveCount = gameState?.moves.length ?? 0;
  const gamePhase = getGamePhase(moveCount);

  const handleLeave = () => {
    disconnect();
    onLeave();
  };

  return (
    <div className="space-y-6 animate-fade-in" role="region" aria-label="Live match spectator mode">
      <div className="flex flex-col gap-4 rounded-xl border border-gray-700/30 bg-gray-800/40 p-4 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white">Spectator Mode</h1>
          <p className="mt-1 text-sm text-gray-400">Watching game {gameId}</p>
        </div>
        <div className="flex flex-wrap items-center gap-3">
          <div
            className={cn(
              "inline-flex items-center gap-2 rounded-full border px-3 py-1 text-sm font-medium capitalize",
              getStatusClasses(status),
            )}
            role="status"
            aria-live="polite"
          >
            {status === "connecting" || status === "reconnecting" ? (
              <LoaderCircle className="h-4 w-4 animate-spin" />
            ) : (
              <Signal className="h-4 w-4" />
            )}
            {status}
          </div>
          <div className="inline-flex items-center gap-2 rounded-full border border-indigo-500/30 bg-indigo-500/15 px-3 py-1 text-sm font-medium text-indigo-300">
            <Eye className="h-4 w-4" />
            {gameState?.spectatorCount ?? 0} spectators
          </div>
          <button
            type="button"
            onClick={handleLeave}
            className="inline-flex items-center rounded-xl border border-gray-700/40 bg-gray-900/60 px-4 py-2 text-sm font-semibold text-white transition-colors hover:bg-gray-800/80"
            aria-label="Leave spectator mode"
          >
            <LogOut className="mr-2 h-4 w-4" />
            Leave
          </button>
        </div>
      </div>

      <div className="flex flex-col lg:flex-row gap-6">
        <div className="flex-1 max-w-[640px] space-y-4">
          <PlayerPanel
            address={gameState?.black.address ?? "Loading..."}
            elo={gameState?.black.elo ?? 0}
            timeRemaining={displayClocks.black}
            isActive={activeColor === "black"}
            color="black"
            capturedPieces={capturedPieces.black}
          />

          <div className="relative rounded-xl border border-gray-700/40 bg-gray-800/60 p-3">
            <div className="mx-auto w-full max-w-[600px]">
              <ChessboardComponent position={gameState?.fen ?? "start"} onDrop={() => false} />
            </div>

            {status === "reconnecting" && (
              <div className="absolute inset-0 flex items-center justify-center rounded-xl bg-black/60 backdrop-blur-sm">
                <div className="rounded-xl border border-amber-500/30 bg-gray-900/80 px-5 py-4 text-center text-amber-300">
                  <LoaderCircle className="mx-auto mb-2 h-6 w-6 animate-spin" />
                  Reconnecting to live feed...
                </div>
              </div>
            )}

            {status === "error" && (
              <div className="absolute inset-0 flex items-center justify-center rounded-xl bg-black/65 backdrop-blur-sm p-4">
                <div className="max-w-xs rounded-xl border border-red-500/30 bg-gray-900/90 p-5 text-center">
                  <AlertTriangle className="mx-auto h-7 w-7 text-red-400" />
                  <p className="mt-3 text-base font-semibold text-white">Spectator feed unavailable</p>
                  <p className="mt-1 text-sm text-gray-400">Try reconnecting to restore the live board.</p>
                  <button
                    type="button"
                    onClick={reconnect}
                    className="mt-4 inline-flex items-center rounded-xl bg-gradient-to-r from-teal-500 to-blue-700 px-4 py-2 text-sm font-semibold text-white hover:from-teal-600 hover:to-blue-800"
                    aria-label="Reconnect spectator feed"
                  >
                    <RefreshCcw className="mr-2 h-4 w-4" />
                    Reconnect
                  </button>
                </div>
              </div>
            )}

            {gameOverCopy && (
              <div className="absolute inset-0 flex items-center justify-center rounded-xl bg-black/70 backdrop-blur-sm p-4">
                <div className="rounded-2xl border border-gray-700/40 bg-gray-900/95 px-6 py-5 text-center shadow-2xl">
                  <p className="text-xs uppercase tracking-[0.22em] text-gray-500">Game complete</p>
                  <h3 className="mt-2 text-2xl font-bold text-white">{gameOverCopy.title}</h3>
                  <p className="mt-2 text-sm text-gray-300">{gameOverCopy.subtitle}</p>
                </div>
              </div>
            )}
          </div>

          <PlayerPanel
            address={gameState?.white.address ?? "Loading..."}
            elo={gameState?.white.elo ?? 0}
            timeRemaining={displayClocks.white}
            isActive={activeColor === "white"}
            color="white"
            capturedPieces={capturedPieces.white}
          />
        </div>

        <div className="w-full lg:w-80 space-y-4">
          <div className="rounded-xl border border-gray-700/40 bg-gray-800/60 p-4" role="region" aria-label="Game info sidebar">
            <h2 className="text-lg font-semibold text-white">Game Info</h2>
            <div className="mt-4 grid gap-3">
              <div className="rounded-xl border border-gray-700/30 bg-gray-900/40 px-3 py-3">
                <p className="text-xs uppercase tracking-[0.18em] text-gray-500">Phase</p>
                <p className="mt-1 text-sm font-semibold capitalize text-white">{gamePhase}</p>
              </div>
              <div className="rounded-xl border border-gray-700/30 bg-gray-900/40 px-3 py-3">
                <p className="text-xs uppercase tracking-[0.18em] text-gray-500">Move Count</p>
                <p className="mt-1 text-sm font-semibold text-white">{moveCount} half-moves</p>
              </div>
              <div className="rounded-xl border border-gray-700/30 bg-gray-900/40 px-3 py-3">
                <p className="text-xs uppercase tracking-[0.18em] text-gray-500">Result</p>
                <p className="mt-1 text-sm font-semibold text-white">{gameState?.result ?? "In Progress"}</p>
              </div>
            </div>
          </div>

          <SpectatorMoveList moves={gameState?.moves ?? []} currentFen={gameState?.fen ?? "start"} />
        </div>
      </div>
    </div>
  );
}
