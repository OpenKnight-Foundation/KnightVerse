"use client";

import React, { useState, useEffect, useCallback } from "react";
import dynamic from "next/dynamic";
import { Chess } from "chess.js";
import { useParams, useRouter } from "next/navigation";
import { useChessSocket } from "@/hook/useChessSocket";
import { FaUser, FaClock, FaSignal } from "react-icons/fa";
import { Web3StatusBar } from "@/components/Web3StatusBar";
import { useCheatDetection } from "@/hook/useCheatDetection";
import { CheatDetectionPanel } from "@/components/chess/CheatDetectionPanel";
import { useIsMobile } from "@/hook/use-mobile";

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
              className={`${
                (Math.floor(i / 8) + (i % 8)) % 2 === 0
                  ? "bg-gray-700/30"
                  : "bg-gray-600/20"
              } rounded-sm shimmer-bg`}
            />
          ))}
        </div>
      </div>
    ),
  },
);

type GameStatus = "playing" | "checkmate" | "stalemate" | "draw" | "resigned";

export default function PlayOnlinePage() {
  const params = useParams();
  const router = useRouter();
  const gameId = params.slug as string;

  const [game] = useState(new Chess());
  const [position, setPosition] = useState("start");
  const [moveHistory, setMoveHistory] = useState<string[]>([]);
  const [whiteTime] = useState(600); // 10 min in seconds
  const [blackTime] = useState(600);
  const [playerColor] = useState<"white" | "black">("white");
  const [gameStatus, setGameStatus] = useState<GameStatus>("playing");
  const [isCheatPanelExpanded, setIsCheatPanelExpanded] = useState(false);
  const [isMoveHistoryOpen, setIsMoveHistoryOpen] = useState(false);
  const isMobile = useIsMobile();

  const {
    status: socketStatus,
    sendMove,
    disconnect,
    reconnect,
    lastOpponentMove,
  } = useChessSocket(gameId);

  const checkGameStatus = useCallback(() => {
    if (game.isCheckmate()) {
      setGameStatus("checkmate");
    } else if (game.isStalemate()) {
      setGameStatus("stalemate");
    } else if (game.isDraw()) {
      setGameStatus("draw");
    }
  }, [game]);

  // Cheat detection
  const {
    opponentAnalysis,
    playerAnalysis,
    recordMove: recordCheatMove,
    reset: resetCheatDetection,
    isActive: isCheatDetectionActive,
  } = useCheatDetection(playerColor);

  // Apply opponent's move to local chess state
  useEffect(() => {
    if (!lastOpponentMove) return;
    try {
      const fenBeforeOpponentMove = game.fen();
      const move = game.move({
        from: lastOpponentMove.from,
        to: lastOpponentMove.to,
        promotion: lastOpponentMove.promotion ?? "q",
      });
      if (move) {
        setPosition(game.fen());
        setMoveHistory((prev: string[]) => [...prev, move.san]);
        recordCheatMove(
          move.san,
          move,
          fenBeforeOpponentMove,
          playerColor === "white" ? "b" : "w",
          Math.ceil(game.moveNumber() / 2),
        );
        checkGameStatus();
      }
    } catch {
      // illegal move from server — ignore
    }
  }, [lastOpponentMove, game, checkGameStatus, recordCheatMove, playerColor]);

  const isMyTurn =
    socketStatus === "connected" &&
    ((playerColor === "white" && game.turn() === "w") ||
      (playerColor === "black" && game.turn() === "b"));

  const handleMove = useCallback(
    ({
      sourceSquare,
      targetSquare,
    }: {
      sourceSquare: string;
      targetSquare: string;
    }) => {
      if (!isMyTurn || gameStatus !== "playing") return false;

      try {
        const fenBeforeMyMove = game.fen();
        const move = game.move({
          from: sourceSquare,
          to: targetSquare,
          promotion: "q",
        });
        if (move === null) return false;

        requestAnimationFrame(() => setPosition(game.fen()));
        setMoveHistory((prev: string[]) => [...prev, move.san]);
        sendMove({ from: sourceSquare, to: targetSquare, promotion: "q" });
        recordCheatMove(
          move.san,
          move,
          fenBeforeMyMove,
          playerColor === "white" ? "w" : "b",
          Math.ceil(game.moveNumber() / 2),
        );
        checkGameStatus();
        return true;
      } catch {
        return false;
      }
    },
    [isMyTurn, game, gameStatus, sendMove, checkGameStatus, recordCheatMove, playerColor],
  );

  const handleResign = useCallback(() => {
    setGameStatus("resigned");
    resetCheatDetection();
    disconnect();
  }, [disconnect, resetCheatDetection]);

  const formatTime = (seconds: number) => {
    const m = Math.floor(seconds / 60);
    const s = seconds % 60;
    return `${m}:${s.toString().padStart(2, "0")}`;
  };

  const socketStatusLabel = () => {
    switch (socketStatus) {
      case "connected":
        return "Live";
      case "connecting":
        return "Connecting...";
      case "reconnecting":
        return "Reconnecting...";
      case "disconnected":
        return "Disconnected";
      case "error":
        return "Error";
      default:
        return "Idle";
    }
  };

  const socketStatusColor = () => {
    switch (socketStatus) {
      case "connected":
        return "text-emerald-400";
      case "connecting":
      case "reconnecting":
        return "text-yellow-400";
      default:
        return "text-red-400";
    }
  };

  // Group moves into pairs for display
  const movePairs = moveHistory.reduce(
    (acc: string[][], move: string, i: number) => {
      if (i % 2 === 0) acc.push([move]);
      else acc[acc.length - 1].push(move);
      return acc;
    },
    [],
  );

  return (
    <div className="min-h-screen p-4 md:p-8" role="region" aria-label="Online Chess Game">
      <div className="max-w-7xl mx-auto">
        {/* Top bar */}
        <div className="flex items-center justify-between mb-4">
          <button
            onClick={() => router.push("/")}
            aria-label="Back to lobby"
            className="text-gray-400 hover:text-white transition-colors text-sm flex items-center gap-2"
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              className="h-4 w-4"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth={2}
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M15 19l-7-7 7-7"
              />
            </svg>
            Back to Lobby
          </button>
          <Web3StatusBar />
        </div>

        <div className="flex flex-col lg:flex-row gap-6 items-start justify-center">
          {/* Chessboard Section */}
          <div className="w-full max-w-[600px]" role="region" aria-label="Chess board">
            {/* Opponent info bar */}
            <div className="flex items-center justify-between mb-3 px-1" role="status" aria-label="Opponent info">
              <div className="flex items-center gap-3">
                <div className="w-8 h-8 rounded-full bg-gradient-to-br from-red-400 to-red-600 flex items-center justify-center">
                  <FaUser className="text-white text-xs" />
                </div>
                <div>
                  <p className="text-sm font-semibold text-white">Opponent</p>
                  <p className="text-xs text-gray-400">
                    {playerColor === "white" ? "Black" : "White"}
                  </p>
                </div>
              </div>
              <div className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-gray-800/60 border border-gray-700/30">
                <FaClock className="text-gray-400 text-xs" />
                <span className="font-mono text-sm text-gray-200">
                  {formatTime(playerColor === "white" ? blackTime : whiteTime)}
                </span>
              </div>
            </div>

            {/* Board */}
            <div className="w-full min-w-[320px]">
              <ChessboardComponent
                position={position}
                onDrop={handleMove}
              />
            </div>

            {/* Player info bar */}
            <div className="flex items-center justify-between mt-3 px-1" role="status" aria-label="Your info">
              <div className="flex items-center gap-3">
                <div className="w-8 h-8 rounded-full bg-gradient-to-br from-teal-400 to-blue-600 flex items-center justify-center">
                  <FaUser className="text-white text-xs" />
                </div>
                <div>
                  <p className="text-sm font-semibold text-white">You</p>
                  <p className="text-xs text-gray-400 capitalize">
                    {playerColor}
                    {isMyTurn && (
                      <span className="ml-2 text-emerald-400">
                        (Your turn)
                      </span>
                    )}
                  </p>
                </div>
              </div>
              <div className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-gray-800/60 border border-gray-700/30">
                <FaClock className="text-gray-400 text-xs" />
                <span className="font-mono text-sm text-gray-200">
                  {formatTime(playerColor === "white" ? whiteTime : blackTime)}
                </span>
              </div>
            </div>

            {/* Mobile controls — visible only on mobile, directly below board */}
            <div className="flex items-center gap-2 mt-3 lg:hidden">
              <button
                onClick={() => {}}
                aria-label="Flip board orientation"
                className="flex-1 py-2.5 rounded-xl bg-gray-800/60 hover:bg-gray-700/60 border border-gray-700/50 text-gray-300 text-sm font-medium transition-all duration-300"
              >
                Flip Board
              </button>
              <button
                onClick={handleResign}
                disabled={gameStatus !== "playing"}
                aria-label="Resign game"
                className="flex-1 py-2.5 rounded-xl bg-red-500/10 hover:bg-red-500/20 border border-red-500/30 text-red-400 text-sm font-medium transition-all duration-300 disabled:opacity-50 disabled:cursor-not-allowed"
              >
                Resign
              </button>
              <div className="flex items-center gap-1.5 px-2 shrink-0">
                <FaSignal className={`text-xs ${socketStatusColor()}`} />
                <span className={`text-xs ${socketStatusColor()}`}>
                  {socketStatusLabel()}
                </span>
              </div>
            </div>
          </div>

          {/* Game Sidebar - Move History & Controls */}
          <div className="w-full lg:w-80 space-y-4" role="complementary" aria-label="Game controls">
            {/* Game Status Card */}
            <div className="rounded-xl border border-gray-700/50 bg-gray-800/40 p-4 animate-fade-in">
              <div className="flex items-center justify-between mb-3">
                <h3 className="text-sm font-semibold text-gray-300">
                  Game Status
                </h3>
                <div className="hidden lg:flex items-center gap-1.5">
                  <FaSignal className={`text-xs ${socketStatusColor()}`} />
                  <span className={`text-xs ${socketStatusColor()}`}>
                    {socketStatusLabel()}
                  </span>
                </div>
              </div>

              {gameStatus !== "playing" && (
                <div className="p-3 rounded-lg bg-gradient-to-r from-yellow-500/10 to-orange-500/10 border border-yellow-500/30 mb-3 animate-scale-in" role="alert" aria-live="assertive">
                  <p className="text-sm font-bold text-yellow-400">
                    {gameStatus === "checkmate" && "Checkmate!"}
                    {gameStatus === "stalemate" && "Stalemate!"}
                    {gameStatus === "draw" && "Draw!"}
                    {gameStatus === "resigned" && "Resigned!"}
                  </p>
                </div>
              )}

              {game.isCheck() && gameStatus === "playing" && (
                <div className="p-2 rounded-lg bg-red-500/10 border border-red-500/30 mb-3 animate-scale-in" role="alert" aria-live="assertive">
                  <p className="text-sm font-bold text-red-400">Check!</p>
                </div>
              )}

              <div className="text-xs text-gray-500">
                Game ID: {gameId?.slice(0, 12)}...
              </div>
            </div>

            {/* Move History */}
            <div className="rounded-xl border border-gray-700/50 bg-gray-800/40 p-4" role="region" aria-label="Move history">
              <button
                className="flex items-center justify-between w-full text-left"
                onClick={() => isMobile && setIsMoveHistoryOpen((prev) => !prev)}
                aria-expanded={isMobile ? isMoveHistoryOpen : true}
              >
                <h3 className="text-sm font-semibold text-gray-300">Moves</h3>
                <span className="text-gray-500 text-xs lg:hidden">
                  {isMoveHistoryOpen ? "▲" : "▼"}
                </span>
              </button>
              <div className={`max-h-64 overflow-y-auto space-y-0.5 mt-3 ${isMobile && !isMoveHistoryOpen ? "hidden" : ""}`}>
                {movePairs.length === 0 ? (
                  <p className="text-xs text-gray-500 italic">
                    No moves yet.{" "}
                    {isMyTurn ? "Your turn to move!" : "Waiting for opponent..."}
                  </p>
                ) : (
                  movePairs.map((pair, i) => (
                    <div
                      key={i}
                      className="flex items-center gap-2 py-1 px-2 rounded hover:bg-gray-700/30 text-sm"
                    >
                      <span className="text-gray-500 w-6 text-right text-xs">
                        {i + 1}.
                      </span>
                      <span className="text-white font-mono text-xs w-16">
                        {pair[0]}
                      </span>
                      <span className="text-gray-400 font-mono text-xs w-16">
                        {pair[1] ?? ""}
                      </span>
                    </div>
                  ))
                )}
              </div>
            </div>

            {/* Cheat Detection Panel */}
            <CheatDetectionPanel
              opponentAnalysis={opponentAnalysis}
              playerAnalysis={playerAnalysis}
              isActive={isCheatDetectionActive}
              isExpanded={isCheatPanelExpanded}
              onToggle={() => setIsCheatPanelExpanded((prev: boolean) => !prev)}
            />

            {/* Controls — desktop only, mobile controls are above the sidebar */}
            <div className="hidden lg:flex gap-2">
              <button
                onClick={() => {}}
                aria-label="Flip board orientation"
                className="flex-1 py-2.5 rounded-xl bg-gray-800/60 hover:bg-gray-700/60 border border-gray-700/50 text-gray-300 text-sm font-medium transition-all duration-300"
              >
                Flip Board
              </button>
              <button
                onClick={handleResign}
                disabled={gameStatus !== "playing"}
                aria-label="Resign game"
                className="flex-1 py-2.5 rounded-xl bg-red-500/10 hover:bg-red-500/20 border border-red-500/30 text-red-400 text-sm font-medium transition-all duration-300 disabled:opacity-50 disabled:cursor-not-allowed"
              >
                Resign
              </button>
            </div>

            {/* Reconnection button */}
            {socketStatus === "disconnected" && gameStatus === "playing" && (
              <button
                onClick={reconnect}
                className="w-full py-2.5 rounded-xl bg-yellow-500/10 hover:bg-yellow-500/20 border border-yellow-500/30 text-yellow-400 text-sm font-medium transition-all duration-300 animate-fade-in"
              >
                Reconnect to Game
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
