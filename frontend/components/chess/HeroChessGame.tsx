"use client";

import React, { useState, useEffect, useCallback } from "react";
import dynamic from "next/dynamic";
import { Chess } from "chess.js";
import { FaUser } from "react-icons/fa";
import { RiAliensFill } from "react-icons/ri";
import { useChessSocket } from "@/hook/useChessSocket";
import { useMatchmaking } from "@/hook/useMatchmaking";
import {
  useStockfishWASM,
  AnalysisResult,
} from "@/components/chess/StockfishWASM";
import { useRouter } from "next/navigation";
import { useMatchmakingContext } from "@/context/matchmakingContext";
import { getChessVariantById } from "@/lib/chessVariants";
import { GameResultOverlay } from "@/components/GameResultOverlay";
import type { GameResult } from "@/components/GameResultOverlay";
import { CapturedPieces } from "@/components/chess/CapturedPieces";
import { EvaluationBar } from "@/components/chess/EvaluationBar";
import MoveTree from "@/components/chess/MoveTree";
import { ErrorBoundary } from "@/components/ErrorBoundary";

const ChessboardComponent = dynamic(
  () => import("@/components/chess/ChessboardComponent"),
  { ssr: false },
);
const GameModeButtons = dynamic(() => import("@/components/GameModeButtons"), {
  ssr: false,
});
const AiPersonalityModal = dynamic(
  () =>
    import("@/app/components/matchmaking/AiPersonalityModal").then((m) => ({
      default: m.AiPersonalityModal,
    })),
  { ssr: false },
);
const MatchmakingModal = dynamic(
  () =>
    import("@/app/components/matchmaking/MatchmakingModal").then((m) => ({
      default: m.MatchmakingModal,
    })),
  { ssr: false },
);
const ChessVariantSelector = dynamic(
  () =>
    import("@/components/ChessVariantSelector").then((m) => ({
      default: m.ChessVariantSelector,
    })),
  { ssr: false },
);

interface HeroChessGameProps {
  onlinePlayerCount: number | null;
  heroBranding: React.ReactNode;
}

export default function HeroChessGame({
  onlinePlayerCount,
  heroBranding,
}: HeroChessGameProps) {
  const [game] = useState(new Chess());
  const [position, setPosition] = useState("start");
  const [viewIndex, setViewIndex] = useState<number | null>(null);
  const [gameMode, setGameMode] = useState<"online" | "bot" | null>(null);
  const router = useRouter();
  const [isPersonalityModalOpen, setIsPersonalityModalOpen] = useState(false);
  const [isMatchmakingModalOpen, setIsMatchmakingModalOpen] = useState(false);
  const [botAnalysis, setBotAnalysis] = useState<AnalysisResult | null>(null);

  const [heroGameResult, setHeroGameResult] = useState<GameResult | null>(null);
  const [heroMoveCount, setHeroMoveCount] = useState(0);

  const { aiPersonality, chessVariant, setChessVariant } =
    useMatchmakingContext();
  const selectedVariant = getChessVariantById(chessVariant);

  const {
    status: matchmakingStatus,
    playerColor,
    error: matchmakingError,
    joinMatchmaking,
    cancelMatchmaking,
    sendMove: matchmakingSendMove,
    lastOpponentMove,
    gameId,
  } = useMatchmaking();

  const {
    status: socketStatus,
    sendMove: socketSendMove,
    disconnect: disconnectSocket,
    reconnect: reconnectSocket,
  } = useChessSocket(gameId);

  const {
    analyzePosition,
    isReady: stockfishReady,
    isAnalyzing,
  } = useStockfishWASM({
    jsBridgePath: "/assets/stockfish.js",
    defaultTimeLimit: 250,
  });

  const currentDisplayPosition = React.useMemo(() => {
    if (viewIndex === null) return position;
    const tempGame = new Chess();
    const history = game.history();
    for (let i = 0; i <= viewIndex; i++) {
      try {
        tempGame.move(history[i]);
      } catch {}
    }
    return tempGame.fen();
  }, [position, viewIndex, game]);

  const handleMoveClick = useCallback(
    (index: number) => {
      setViewIndex(index === game.history().length - 1 ? null : index);
    },
    [game],
  );

  const sendMove = useCallback(
    (from: string, to: string, promotion?: string) => {
      if (gameId) {
        socketSendMove({ from, to, promotion: promotion || "q" });
      } else {
        matchmakingSendMove(from, to, promotion);
      }
    },
    [gameId, socketSendMove, matchmakingSendMove],
  );

  useEffect(() => {
    if (matchmakingStatus === "match_found" && gameId) {
      router.push(`/play/${gameId}`);
    }
  }, [matchmakingStatus, gameId, router]);

  useEffect(() => {
    if (!lastOpponentMove || gameMode !== "online") return;
    try {
      const move = game.move({
        from: lastOpponentMove.from,
        to: lastOpponentMove.to,
        promotion: lastOpponentMove.promotion ?? "q",
      });
      if (move) {
        setPosition(game.fen());
        setViewIndex(null);
      }
    } catch {
      // illegal move from server — ignore
    }
  }, [lastOpponentMove, game, gameMode]);

  useEffect(() => {
    let active = true;

    const playBotMove = async () => {
      const isHeroMode = gameMode === null;
      const isBotMode = gameMode === "bot";
      if (!stockfishReady || isAnalyzing || !(isHeroMode || isBotMode)) return;
      if (game.turn() !== "b" || game.isGameOver()) return;

      try {
        let depth = 5;
        if (isBotMode) {
          depth = 10;
          if (aiPersonality === "aggressive") depth = 15;
          if (aiPersonality === "defensive") depth = 18;
        }

        const fenBeforeAnalysis = game.fen();
        const result = await analyzePosition(fenBeforeAnalysis, depth);
        if (
          active &&
          result.bestMove &&
          game.fen() === fenBeforeAnalysis &&
          !game.isGameOver()
        ) {
          setBotAnalysis(result);
          const from = result.bestMove.substring(0, 2);
          const to = result.bestMove.substring(2, 4);
          const promotion =
            result.bestMove.length > 4
              ? result.bestMove.substring(4, 5)
              : undefined;
          game.move({ from, to, promotion });
          setPosition(game.fen());
          setViewIndex(null);
          setHeroMoveCount((c) => c + 1);
        }
      } catch (e) {
        console.error("Bot failed to move:", e);
      }
    };

    const timer = setTimeout(playBotMove, 400);
    return () => {
      active = false;
      clearTimeout(timer);
    };
  }, [
    position,
    gameMode,
    analyzePosition,
    aiPersonality,
    game,
    stockfishReady,
    isAnalyzing,
  ]);

  useEffect(() => {
    if (!game.isGameOver()) return;
    if (gameMode !== null && gameMode !== "bot") return;

    if (game.isCheckmate()) {
      setHeroGameResult(game.turn() === "w" ? "black_wins" : "white_wins");
    } else if (game.isStalemate()) {
      setHeroGameResult("stalemate");
    } else {
      setHeroGameResult("draw");
    }
  }, [position, game, gameMode]);

  const isMyTurn = (() => {
    if (gameMode === null) {
      return game.turn() === "w";
    }
    if (gameMode === "bot") {
      return game.turn() === "w";
    }
    if (gameMode === "online") {
      return (
        socketStatus === "connected" &&
        ((playerColor === "white" && game.turn() === "w") ||
          (playerColor === "black" && game.turn() === "b"))
      );
    }
    return true;
  })();

  const makeMove = useCallback(
    (move: { from: string; to: string; promotion?: string }) => {
      try {
        const moveResult = game.move(move);
        if (moveResult === null) return false;

        setBotAnalysis(null);
        setHeroMoveCount((c) => c + 1);
        requestAnimationFrame(() => setPosition(game.fen()));
        if (gameMode === "online") {
          sendMove(move.from, move.to, move.promotion);
        }

        setPosition(game.fen());
        setViewIndex(null);
        return true;
      } catch {
        return false;
      }
    },
    [game, gameMode, sendMove],
  );

  const handleMove = useCallback(
    ({
      sourceSquare,
      targetSquare,
    }: {
      sourceSquare: string;
      targetSquare: string;
    }) => {
      if (!isMyTurn || game.isGameOver()) return false;

      const history = game.history({ verbose: true });
      const currentMoveIndex = viewIndex ?? history.length - 1;

      if (viewIndex !== null && viewIndex < history.length - 1) {
        const parentMove = history[currentMoveIndex] as typeof history[number] & {
          variations?: Array<{ san: string }[]>;
        };
        if (!parentMove.variations) {
          parentMove.variations = [];
        }

        const tempGame = new Chess(parentMove.after);
        const move = tempGame.move({
          from: sourceSquare,
          to: targetSquare,
          promotion: "q",
        });

        if (move) {
          if (!parentMove.variations.some((v) => v[0]?.san === move.san)) {
            parentMove.variations.push([move]);
          }
          game.load(tempGame.fen());
          setPosition(game.fen());
          setViewIndex(null);
        }
      } else {
        makeMove({ from: sourceSquare, to: targetSquare, promotion: "q" });
      }
      return true;
    },
    [game, isMyTurn, viewIndex, makeMove],
  );

  const handleHeroPlayAgain = useCallback(() => {
    game.reset();
    setPosition("start");
    setBotAnalysis(null);
    setHeroGameResult(null);
    setHeroMoveCount(0);
  }, [game]);

  const handleExit = () => {
    if (gameMode === "online") {
      cancelMatchmaking();
      disconnectSocket();
    }
    game.reset();
    setPosition("start");
    setGameMode(null);
    setBotAnalysis(null);
    setHeroGameResult(null);
    setHeroMoveCount(0);
  };

  const handleSetGameMode = (mode: "online" | "bot" | null) => {
    if (mode === "online") {
      setGameMode(mode);
      setIsMatchmakingModalOpen(true);
    } else if (mode === "bot") {
      setGameMode(mode);
      setIsPersonalityModalOpen(true);
    } else {
      setGameMode(mode);
    }
  };

  const handleMatchmakingConfirm = (type: "Rated" | "Casual") => {
    setIsMatchmakingModalOpen(false);
    joinMatchmaking(type, chessVariant);
  };

  const handleMatchmakingClose = () => {
    setIsMatchmakingModalOpen(false);
    setGameMode(null);
  };

  const handlePersonalityConfirm = () => {
    setIsPersonalityModalOpen(false);
    game.reset();
    setPosition("start");
    setBotAnalysis(null);
    setHeroGameResult(null);
    setHeroMoveCount(0);
  };

  const handlePersonalityClose = () => {
    setIsPersonalityModalOpen(false);
    setGameMode(null);
  };

  const handlePlayOnlineFromResult = () => {
    setHeroGameResult(null);
    handleSetGameMode("online");
  };

  const onlineStatusLabel = () => {
    if (socketStatus === "reconnecting") return "🔄 Reconnecting...";
    if (matchmakingStatus === "match_found") return "✅ Match found! Starting…";
    if (socketStatus === "connected")
      return `🟢 Online Match (you are ${playerColor})`;
    if (matchmakingStatus === "error" || socketStatus === "error")
      return `❌ ${matchmakingError ?? "Connection error"}`;
    return "Online Match";
  };

  return (
    <div className="min-h-screen p-4 md:p-8" role="region" aria-label="Home">
      <div className="max-w-7xl mx-auto">
        <div className="flex flex-col lg:flex-row gap-8 items-center justify-center min-h-[80vh]">
          <div
            className="w-full max-w-[560px] order-2 lg:order-1"
            role="region"
            aria-label="Interactive Chessboard"
          >
            <div className="flex items-center justify-between mb-3 px-1">
              <div className="flex items-center gap-2">
                <div
                  className={`w-3 h-3 rounded-full transition-colors duration-300 ${
                    game.turn() === "w"
                      ? "bg-white shadow-lg shadow-white/30"
                      : "bg-gray-600"
                  }`}
                />
                <span
                  className={`text-sm font-medium transition-colors duration-300 ${
                    game.turn() === "w" ? "text-white" : "text-gray-500"
                  }`}
                >
                  You (White)
                </span>
              </div>

              {botAnalysis && (
                <div className="flex items-center gap-3 text-xs font-mono text-teal-400/80">
                  <span>
                    Eval:{" "}
                    {botAnalysis.evaluation != null
                      ? `${botAnalysis.evaluation > 0 ? "+" : ""}${botAnalysis.evaluation.toFixed(2)}`
                      : "N/A"}
                  </span>
                  <span className="opacity-40">|</span>
                  <span>D{botAnalysis.depth}</span>
                </div>
              )}

              <div className="flex items-center gap-2">
                <span
                  className={`text-sm font-medium transition-colors duration-300 ${
                    game.turn() === "b" ? "text-gray-200" : "text-gray-500"
                  }`}
                >
                  Bot (Black)
                </span>
                <div
                  className={`w-3 h-3 rounded-full transition-colors duration-300 ${
                    game.turn() === "b"
                      ? "bg-gray-300 shadow-lg shadow-gray-300/30 animate-pulse"
                      : "bg-gray-700"
                  }`}
                />
              </div>
            </div>

            <div className="mb-2 px-1">
              <CapturedPieces fen={position} color="black" />
            </div>

            <div className="w-full min-w-[320px] flex flex-row items-stretch justify-center gap-2 md:gap-4">
              <EvaluationBar
                evaluation={botAnalysis?.evaluation ?? null}
                mate={botAnalysis?.mate ?? null}
                isFlipped={false}
              />
              <div className="w-full">
                <ErrorBoundary componentName="Chessboard">
                  <ChessboardComponent
                    position={currentDisplayPosition}
                    onDrop={handleMove}
                    isMyTurn={isMyTurn}
                    aria-label="Chess board. You play as White. Click or drag pieces to move."
                  />
                </ErrorBoundary>
              </div>
            </div>

            <div className="mt-2 px-1">
              <CapturedPieces fen={position} color="white" />
            </div>

            <div className="flex items-center justify-between mt-3 px-1 text-xs text-gray-500">
              <span>
                {heroMoveCount > 0
                  ? `Move ${Math.ceil(heroMoveCount / 2)}`
                  : "Make your first move!"}
              </span>
              {!stockfishReady && (
                <span className="flex items-center gap-1.5 text-yellow-500">
                  <span className="w-1.5 h-1.5 rounded-full bg-yellow-500 animate-pulse" />
                  Loading engine...
                </span>
              )}
              {stockfishReady && heroMoveCount === 0 && (
                <span className="flex items-center gap-1.5 text-emerald-500">
                  <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 animate-pulse-glow" />
                  Engine ready
                </span>
              )}
            </div>
          </div>

          <div className="w-full max-w-[280px] order-3 flex justify-center mt-4 lg:mt-0">
            <MoveTree
              history={game.history({ verbose: true })}
              onMoveClick={handleMoveClick}
              currentMoveIndex={viewIndex ?? game.history().length - 1}
            />
          </div>

          <div
            className="flex flex-col justify-center max-w-md w-full order-1 lg:order-2"
            role="region"
            aria-label="KnightVerse information"
          >
            {heroBranding}
          </div>
        </div>
      </div>

      <div className="max-w-7xl mx-auto mt-16 md:mt-24">
        <div className="text-center mb-8 animate-fade-in">
          <h2 className="text-2xl md:text-3xl font-bold text-white">
            Choose Your Arena
          </h2>
          <p className="text-gray-400 text-sm mt-2">
            Play online, train with bots, or solve puzzles to earn XLM
          </p>
        </div>

        <div className="flex flex-col md:flex-row gap-8 items-start justify-center">
          <div
            className="flex flex-col justify-center space-y-6 max-w-[500px] w-full"
            role="region"
            aria-label="Game mode selection"
          >
            <GameModeButtons setGameMode={handleSetGameMode} />
            <ChessVariantSelector
              selectedVariant={chessVariant}
              onSelect={setChessVariant}
            />
          </div>
        </div>
      </div>

      {gameMode && (
        <div
          className="fixed bottom-4 left-1/2 -translate-x-1/2 z-40 flex items-center justify-between bg-gray-900/95 backdrop-blur-sm p-4 rounded-2xl border border-gray-700/50 shadow-2xl animate-slide-up max-w-lg w-[calc(100%-2rem)]"
          role="status"
          aria-live="polite"
        >
          <div className="flex items-center gap-3">
            <div className="bg-gradient-to-br from-teal-400/30 to-blue-500/30 p-2.5 rounded-xl">
              {gameMode === "online" ? (
                <FaUser className="text-xl text-white" />
              ) : (
                <RiAliensFill className="text-xl text-white" />
              )}
            </div>
            <div>
              <h2 className="text-base font-bold text-white">
                {gameMode === "online" ? onlineStatusLabel() : "Playing vs Bot"}
              </h2>
              <p className="text-xs text-cyan-100/70">
                {selectedVariant.label} / {selectedVariant.averageGameTime}
              </p>
            </div>
          </div>
          <button
            onClick={handleExit}
            className="px-4 py-2 bg-red-500/10 hover:bg-red-500/20 border border-red-500/30 hover:border-red-400/50 rounded-xl text-white font-medium transition-all duration-300 hover:scale-[1.02] active:scale-[0.98] text-sm"
          >
            Exit
          </button>
        </div>
      )}

      {gameMode === "online" && matchmakingStatus === "searching" && (
        <div
          className="fixed inset-0 bg-black/80 backdrop-blur-sm z-50 flex items-center justify-center animate-overlay-in"
          role="dialog"
          aria-modal="true"
          aria-label="Searching for opponent"
        >
          <div className="bg-gray-900 p-8 rounded-2xl border border-yellow-500/30 text-center animate-modal-in max-w-sm w-full mx-4">
            <div className="flex flex-col items-center gap-4">
              <h3 className="text-xl font-bold text-yellow-400">
                Looking for opponent...
              </h3>
              <span className="relative flex h-10 w-10">
                <span className="animate-ping absolute inline-flex h-full w-full rounded-full border-2 border-yellow-400 opacity-75 [animation-duration:0.8s]" />
                <span className="relative inline-flex rounded-full h-10 w-10 border-2 border-yellow-500 bg-yellow-500/10" />
              </span>
              <p className="text-gray-300 text-sm" aria-live="polite">
                {onlinePlayerCount} Players online
              </p>
              <p className="text-cyan-100 text-xs uppercase tracking-[0.24em]">
                Queueing for {selectedVariant.label}
              </p>
              <button
                onClick={handleExit}
                className="w-full px-4 py-2.5 bg-red-500/10 hover:bg-red-500/20 border border-red-500/30 hover:border-red-400/50 rounded-xl text-white font-medium transition-all duration-300 hover:scale-[1.02] active:scale-[0.98]"
              >
                Cancel Search
              </button>
            </div>
          </div>
        </div>
      )}

      {gameMode === "online" && socketStatus === "reconnecting" && (
        <div
          className="fixed inset-0 bg-black/80 backdrop-blur-sm z-50 flex items-center justify-center animate-overlay-in"
          role="dialog"
          aria-modal="true"
          aria-label="Reconnecting to game"
        >
          <div className="bg-gray-900 p-8 rounded-2xl border border-yellow-500/30 text-center animate-modal-in max-w-sm w-full mx-4">
            <div className="flex flex-col items-center gap-4">
              <div className="w-10 h-10 rounded-full border-2 border-yellow-500 border-t-transparent animate-spin" />
              <h3 className="text-xl font-bold text-yellow-400">
                Reconnecting...
              </h3>
              <p className="text-gray-300 text-sm">
                Attempting to restore connection
              </p>
              <button
                onClick={reconnectSocket}
                className="w-full px-4 py-2.5 bg-yellow-500/10 hover:bg-yellow-500/20 border border-yellow-500/30 rounded-xl text-yellow-400 font-medium transition-all duration-300 hover:scale-[1.02] active:scale-[0.98]"
              >
                Reconnect Now
              </button>
            </div>
          </div>
        </div>
      )}

      {heroGameResult && (
        <GameResultOverlay
          result={heroGameResult}
          onPlayAgain={handleHeroPlayAgain}
          onPlayOnline={handlePlayOnlineFromResult}
        />
      )}

      <MatchmakingModal
        isOpen={isMatchmakingModalOpen}
        onClose={handleMatchmakingClose}
        onConfirm={handleMatchmakingConfirm}
      />
      <AiPersonalityModal
        isOpen={isPersonalityModalOpen}
        onClose={handlePersonalityClose}
        onConfirm={handlePersonalityConfirm}
      />
    </div>
  );
}
