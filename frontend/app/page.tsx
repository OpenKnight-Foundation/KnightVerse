"use client";

import React, { useState, useEffect, useCallback } from "react";
import dynamic from "next/dynamic";
const ChessboardComponent = dynamic(
  () => import("@/components/chess/ChessboardComponent"),
  { ssr: false },
);
import { Chess } from "chess.js";
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
import { FaUser } from "react-icons/fa";
import { RiAliensFill } from "react-icons/ri";
import { useChessSocket } from "@/hook/useChessSocket";
import { useMatchmaking } from "@/hook/useMatchmaking";
import { useStockfishWASM, AnalysisResult } from "@/components/chess/StockfishWASM";
import { useRouter } from "next/navigation";
import { useMatchmakingContext } from "@/context/matchmakingContext";
import { ChessVariantSelector } from "@/components/ChessVariantSelector";
import { getChessVariantById } from "@/lib/chessVariants";
import { HeroBranding } from "@/components/HeroBranding";
import { GameResultOverlay } from "@/components/GameResultOverlay";
import type { GameResult } from "@/components/GameResultOverlay";
import { WalletConnectModal } from "@/components/WalletConnectModal";
import { CapturedPieces } from "@/components/chess/CapturedPieces";

export default function Home() {
  const [game] = useState(new Chess());
  const [position, setPosition] = useState("start");
  const [gameMode, setGameMode] = useState<"online" | "bot" | null>(null);
  const router = useRouter();
  const [onlinePlayerCount, setOnlinePlayerCount] = useState<number | null>(null);
  const API_BASE = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8000";
  const PLAYER_COUNT_ENDPOINT = `${API_BASE}/v1/players/online`;
  const [isPersonalityModalOpen, setIsPersonalityModalOpen] = useState(false);
  const [isMatchmakingModalOpen, setIsMatchmakingModalOpen] = useState(false);
  const [isWalletModalOpen, setIsWalletModalOpen] = useState(false);
  const [botAnalysis, setBotAnalysis] = useState<AnalysisResult | null>(null);

  // Hero game state (instant-play on landing)
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

  const { analyzePosition, isReady: stockfishReady, isAnalyzing } = useStockfishWASM({
    jsBridgePath: "/assets/stockfish.js",
    defaultTimeLimit: 250,
  });

  // Choose which sendMove to use based on game state
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

  // Kick off matchmaking → route to game
  useEffect(() => {
    if (matchmakingStatus === "match_found" && gameId) {
      router.push(`/play/${gameId}`);
    }
  }, [matchmakingStatus, gameId, router]);

  // Apply opponent's move in online mode
  useEffect(() => {
    if (!lastOpponentMove || gameMode !== "online") return;
    try {
      const move = game.move({
        from: lastOpponentMove.from,
        to: lastOpponentMove.to,
        promotion: lastOpponentMove.promotion ?? "q",
      });
      if (move) setPosition(game.fen());
    } catch {
      // illegal move from server — ignore
    }
  }, [lastOpponentMove, game, gameMode]);

  // Fetch online player count
  useEffect(() => {
    let isMounted = true;
    const fetchOnlinePlayers = async () => {
      try {
        const resp = await fetch(PLAYER_COUNT_ENDPOINT);
        if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
        const data = (await resp.json()) as { count?: number };
        if (isMounted && typeof data.count === "number") {
          setOnlinePlayerCount(data.count);
        }
      } catch {
        console.warn("[KnightVerse] Could not fetch online player count — backend offline?");
        if (isMounted) setOnlinePlayerCount(null);
      }
    };

    fetchOnlinePlayers();
    const intervalId = window.setInterval(fetchOnlinePlayers, 20000);
    return () => {
      isMounted = false;
      clearInterval(intervalId);
    };
  }, [PLAYER_COUNT_ENDPOINT]);

  // ─── HERO BOT: Stockfish auto-plays Black at easy depth ───
  useEffect(() => {
    let active = true;

    const playBotMove = async () => {
      // Only respond when:
      // - No explicit game mode selected (hero mode) OR game mode is "bot"
      // - It's Black's turn
      // - Stockfish is ready
      // - Game is not over
      // - Not currently analyzing
      const isHeroMode = gameMode === null;
      const isBotMode = gameMode === "bot";
      if (!stockfishReady || isAnalyzing || !(isHeroMode || isBotMode)) return;
      if (game.turn() !== "b" || game.isGameOver()) return;

      try {
        // Easy depth for hero, deeper for explicit bot mode
        let depth = 5; // easy for landing page
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
            result.bestMove.length > 4 ? result.bestMove.substring(4, 5) : undefined;
          game.move({ from, to, promotion });
          setPosition(game.fen());
          setHeroMoveCount((c) => c + 1);
        }
      } catch (e) {
        console.error("Bot failed to move:", e);
      }
    };

    // Small delay so the board animates the human move first
    const timer = setTimeout(playBotMove, 400);
    return () => {
      active = false;
      clearTimeout(timer);
    };
  }, [position, gameMode, analyzePosition, aiPersonality, game, stockfishReady, isAnalyzing]);

  // ─── DETECT GAME OVER for hero / bot mode ───
  useEffect(() => {
    if (!game.isGameOver()) return;
    // Only show result overlay for hero mode or bot mode
    if (gameMode !== null && gameMode !== "bot") return;

    if (game.isCheckmate()) {
      // The side whose turn it is has been checkmated
      setHeroGameResult(game.turn() === "w" ? "black_wins" : "white_wins");
    } else if (game.isStalemate()) {
      setHeroGameResult("stalemate");
    } else {
      setHeroGameResult("draw");
    }
  }, [position, game, gameMode]);

  // ─── HERO MOVE HANDLER (always active) ───
  const isMyTurn = (() => {
    if (gameMode === null) {
      // Hero mode: White's turn only
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

  const handleMove = useCallback(
    ({
      sourceSquare,
      targetSquare,
    }: {
      sourceSquare: string;
      targetSquare: string;
    }) => {
      if (!isMyTurn || game.isGameOver()) return false;

      try {
        const move = game.move({
          from: sourceSquare,
          to: targetSquare,
          promotion: "q",
        });
        if (move === null) return false;

        setBotAnalysis(null);
        setHeroMoveCount((c) => c + 1);
        requestAnimationFrame(() => setPosition(game.fen()));

        // Forward move to server in online mode
        if (gameMode === "online") {
          sendMove(sourceSquare, targetSquare, "q");
        }

        return true;
      } catch {
        return false;
      }
    },
    [isMyTurn, game, gameMode, sendMove],
  );

  // ─── HERO PLAY AGAIN ───
  const handleHeroPlayAgain = useCallback(() => {
    game.reset();
    setPosition("start");
    setBotAnalysis(null);
    setHeroGameResult(null);
    setHeroMoveCount(0);
  }, [game]);

  // ─── MODE SELECTION (below the fold) ───
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
    joinMatchmaking(type);
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

  // Online status overlay label
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
      {/* ═══════════════════════════════════════════════════════════ */}
      {/*  HERO SECTION — Instant Play + Stellar Branding           */}
      {/* ═══════════════════════════════════════════════════════════ */}
      <div className="max-w-7xl mx-auto">
        <div className="flex flex-col lg:flex-row gap-8 items-center justify-center min-h-[80vh]">
          {/* Chessboard — always interactive */}
          <div
            className="w-full max-w-[560px] order-2 lg:order-1"
            role="region"
            aria-label="Interactive Chessboard"
          >
            {/* Turn indicator */}
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

            {/* Captured Pieces by Bot (Black capturing White) */}
            <div className="mb-2 px-1">
              <CapturedPieces fen={position} color="black" />
            </div>

            {/* The board */}
            <div className="w-full min-w-[320px]">
              <ChessboardComponent
                position={position}
                onDrop={handleMove}
                aria-label="Chess board. You play as White. Click or drag pieces to move."
              />
            </div>

            {/* Captured Pieces by You (White capturing Black) */}
            <div className="mt-2 px-1">
              <CapturedPieces fen={position} color="white" />
            </div>

            {/* Move counter */}
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

          {/* Branding Panel */}
          <div
            className="flex flex-col justify-center max-w-md w-full order-1 lg:order-2"
            role="region"
            aria-label="KnightVerse information"
          >
            <HeroBranding
              onlinePlayerCount={onlinePlayerCount}
              onConnectWallet={() => setIsWalletModalOpen(true)}
            />
          </div>
        </div>
      </div>

      {/* ═══════════════════════════════════════════════════════════ */}
      {/*  GAME MODES SECTION — Below the fold                      */}
      {/* ═══════════════════════════════════════════════════════════ */}
      <div className="max-w-7xl mx-auto mt-16 md:mt-24">
        {/* Section header */}
        <div className="text-center mb-8 animate-fade-in">
          <h2 className="text-2xl md:text-3xl font-bold text-white">
            Choose Your Arena
          </h2>
          <p className="text-gray-400 text-sm mt-2">
            Play online, train with bots, or solve puzzles to earn XLM
          </p>
        </div>

        <div className="flex flex-col md:flex-row gap-8 items-start justify-center">
          {/* Game Mode Buttons */}
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

      {/* ═══════════════════════════════════════════════════════════ */}
      {/*  ACTIVE GAME OVERLAYS                                     */}
      {/* ═══════════════════════════════════════════════════════════ */}

      {/* Game mode active bar */}
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
                {gameMode === "online"
                  ? onlineStatusLabel()
                  : "Playing vs Bot"}
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

      {/* Searching overlay */}
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

      {/* Reconnecting overlay */}
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

      {/* Game Result Overlay */}
      {heroGameResult && (
        <GameResultOverlay
          result={heroGameResult}
          onPlayAgain={handleHeroPlayAgain}
          onPlayOnline={handlePlayOnlineFromResult}
        />
      )}

      {/* Modals */}
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
      <WalletConnectModal
        isOpen={isWalletModalOpen}
        onClose={() => setIsWalletModalOpen(false)}
      />
    </div>
  );
}
