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
import { Web3StatusBar } from "@/components/Web3StatusBar";
import { ChessVariantSelector } from "@/components/ChessVariantSelector";
import { getChessVariantById } from "@/lib/chessVariants";

export default function Home() {
  const [game] = useState(new Chess());
  const [position, setPosition] = useState("start");
  const [gameMode, setGameMode] = useState<"online" | "bot" | null>(null);
  const router = useRouter();
  const [onlinePlayerCount, setOnlinePlayerCount] = useState<number | null>(
    null,
  );
  const API_BASE = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8000";
  const PLAYER_COUNT_ENDPOINT = `${API_BASE}/v1/players/online`;
  const [isPersonalityModalOpen, setIsPersonalityModalOpen] = useState(false);
  const [isMatchmakingModalOpen, setIsMatchmakingModalOpen] = useState(false);
  const [botAnalysis, setBotAnalysis] = useState<AnalysisResult | null>(null);

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

  const { analyzePosition, isReady: stockfishReady } = useStockfishWASM({
    jsBridgePath: "/assets/stockfish.js",
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

  // Kick off matchmaking when online mode is selected — but only after personality is confirmed
  // (joinMatchmaking is now called from handlePersonalityConfirm, not here)

  useEffect(() => {
    let isMounted = true;
    const fetchOnlinePlayers = async () => {
      try {
        const resp = await fetch(PLAYER_COUNT_ENDPOINT);
        if (!resp.ok) {
          throw new Error(`HTTP ${resp.status}`);
        }
        const data = (await resp.json()) as { count?: number };
        if (isMounted && typeof data.count === "number") {
          setOnlinePlayerCount(data.count);
        }
      } catch (err) {
        console.error("Failed to fetch player count", err);
        if (isMounted) {
          setOnlinePlayerCount(null);
        }
      }
    };

    fetchOnlinePlayers();
    const intervalId = window.setInterval(fetchOnlinePlayers, 20000);

    return () => {
      isMounted = false;
      clearInterval(intervalId);
    };
  }, [PLAYER_COUNT_ENDPOINT]);

  useEffect(() => {
    if (matchmakingStatus === "match_found" && gameId) {
      router.push(`/play/${gameId}`);
    }
  }, [matchmakingStatus, gameId, router]);

  // Apply opponent's move to local chess state
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

  // Bot logic
  useEffect(() => {
    let active = true;
    const playBotMove = async () => {
      if (!stockfishReady || gameMode !== "bot" || game.turn() !== "b" || game.isGameOver()) return;
      try {
        let depth = 10;
        if (aiPersonality === "aggressive") depth = 15;
        if (aiPersonality === "defensive") depth = 18;

        // Capture FEN before the async call; discard result if the position changed
        const fenBeforeAnalysis = game.fen();
        const result = await analyzePosition(fenBeforeAnalysis, depth);
        if (active && result.bestMove && game.fen() === fenBeforeAnalysis && !game.isGameOver()) {
          setBotAnalysis(result);
          const from = result.bestMove.substring(0, 2);
          const to = result.bestMove.substring(2, 4);
          const promotion = result.bestMove.length > 4 ? result.bestMove.substring(4, 5) : undefined;
          game.move({ from, to, promotion });
          setPosition(game.fen());
        }
      } catch (e) {
        console.error("Bot failed to move:", e);
      }
    };
    playBotMove();
    return () => { active = false; };
  }, [position, gameMode, analyzePosition, aiPersonality, game, stockfishReady]);

  const isMyTurn =
    (gameMode === "bot" ? game.turn() === "w" : true) &&
    (gameMode !== "online" ||
      (socketStatus === "connected" &&
        ((playerColor === "white" && game.turn() === "w") ||
          (playerColor === "black" && game.turn() === "b"))));

  const handleMove = useCallback(({
    sourceSquare,
    targetSquare,
  }: {
    sourceSquare: string;
    targetSquare: string;
  }) => {
    if (!isMyTurn) return false;

    try {
      const move = game.move({
        from: sourceSquare,
        to: targetSquare,
        promotion: "q",
      });
      if (move === null) return false;

      setBotAnalysis(null);
      requestAnimationFrame(() => setPosition(game.fen()));

      // Forward move to server in online mode
      if (gameMode === "online") {
        sendMove(sourceSquare, targetSquare, "q");
      }

      return true;
    } catch {
      return false;
    }
  }, [isMyTurn, game, gameMode, sendMove]);

  const handleExit = () => {
    if (gameMode === "online") {
      cancelMatchmaking();
      disconnectSocket();
    }
    game.reset();
    setPosition("start");
    setGameMode(null);
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
  };

  const handlePersonalityClose = () => {
    setIsPersonalityModalOpen(false);
    setGameMode(null);
  };

  // Searching / waiting overlay label
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
        <div className="flex flex-col md:flex-row gap-8 items-center justify-center">
          {/* Chessboard Section */}
          <div className="w-full max-w-[600px] order-2 md:order-1" role="region" aria-label="Chessboard">
            <div className="w-full min-w-[320px]">
              <ChessboardComponent position={position} onDrop={handleMove} aria-label="Chess board. Use arrow keys or mouse to interact." />
            </div>

            {gameMode && (
              <div className="mt-4 flex items-center justify-between bg-gray-800/60 p-4 rounded-xl border border-gray-700/50 animate-slide-up" role="status" aria-live="polite">
                <div className="flex items-center gap-4">
                  <div className="bg-gradient-to-br from-teal-400/30 to-blue-500/30 p-3 rounded-xl">
                    {gameMode === "online" ? (
                      <FaUser className="text-2xl text-white filter drop-shadow-md" />
                    ) : (
                      <RiAliensFill className="text-2xl text-white filter drop-shadow-md" />
                    )}
                  </div>
                  <div>
                    <h2 className="text-xl font-bold text-white tracking-wide">
                      {gameMode === "online"
                        ? onlineStatusLabel()
                        : "Playing vs Bot"}
                    </h2>
                    <p className="mt-1 text-sm text-cyan-100/80">
                      Variant: {selectedVariant.label} / {selectedVariant.averageGameTime}
                    </p>
                  </div>
                </div>

                {gameMode === "bot" && botAnalysis && (
                  <div className="flex items-center gap-4 text-xs font-mono text-teal-400">
                    <span>Eval: {botAnalysis.evaluation != null ? `${botAnalysis.evaluation > 0 ? "+" : ""}${botAnalysis.evaluation.toFixed(2)}` : "N/A"}</span>
                    <span className="opacity-50">|</span>
                    <span>Depth: {botAnalysis.depth}</span>
                  </div>
                )}

                <button
                  onClick={handleExit}
                  className="px-4 py-2 bg-red-500/10 hover:bg-red-500/20 border border-red-500/30 hover:border-red-400/50 rounded-xl text-white font-medium transition-all duration-300 hover:scale-[1.02] active:scale-[0.98] text-sm"
                >
                  Exit Game
                </button>
              </div>
            )}

            {gameMode === "online" && matchmakingStatus === "searching" && (
              <div className="fixed inset-0 bg-black/80 backdrop-blur-sm z-50 flex items-center justify-center animate-overlay-in" role="dialog" aria-modal="true" aria-label="Searching for opponent">
                <div className="bg-gray-900 p-8 rounded-2xl border border-yellow-500/30 text-center animate-modal-in max-w-sm w-full mx-4">
                  <div className="flex flex-col items-center gap-4">
                    <h3 className="text-xl font-bold text-yellow-400">
                      Looking for opponent...
                    </h3>
                    <span className="relative flex h-10 w-10">
                      <span className="animate-ping absolute inline-flex h-full w-full rounded-full border-2 border-yellow-400 opacity-75 [animation-duration:0.8s]"></span>
                      <span className="relative inline-flex rounded-full h-10 w-10 border-2 border-yellow-500 bg-yellow-500/10"></span>
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
              <div className="fixed inset-0 bg-black/80 backdrop-blur-sm z-50 flex items-center justify-center animate-overlay-in" role="dialog" aria-modal="true" aria-label="Reconnecting to game">
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
          </div>

          {/* Game Modes Section */}
          <div className="flex flex-col justify-center space-y-6 max-w-[500px] w-full order-1 md:order-2" role="region" aria-label="Game mode selection">
            {!gameMode && (
              <>
                <GameModeButtons setGameMode={handleSetGameMode} />
                <ChessVariantSelector
                  selectedVariant={chessVariant}
                  onSelect={setChessVariant}
                />
              </>
            )}
          </div>
        </div>
      </div>

      {/* Matchmaking Selection Modal */}
      <MatchmakingModal
        isOpen={isMatchmakingModalOpen}
        onClose={handleMatchmakingClose}
        onConfirm={handleMatchmakingConfirm}
      />

      {/* AI Personality Selection Modal */}
      <AiPersonalityModal
        isOpen={isPersonalityModalOpen}
        onClose={handlePersonalityClose}
        onConfirm={handlePersonalityConfirm}
      />
    </div>
  );
}
