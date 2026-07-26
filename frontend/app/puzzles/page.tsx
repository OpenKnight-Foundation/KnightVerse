"use client";

import React, { useState, useCallback } from "react";
import dynamic from "next/dynamic";
import { Chess } from "chess.js";
import { FaTrophy, FaStar, FaCheck, FaTimes, FaRedo, FaArrowLeft } from "react-icons/fa";
import { useAppContext } from "@/context/walletContext";
import { useToast } from "@/components/ui/toast";
import { Web3StatusBar } from "@/components/Web3StatusBar";
import { useTrackedTransaction } from "@/hook/useTrackedTransaction";

const ChessboardComponent = dynamic(
  () => import("@/components/chess/ChessboardComponent"),
  { ssr: false },
);

interface Puzzle {
  id: number;
  fen: string;
  title: string;
  difficulty: "easy" | "medium" | "hard";
  description: string;
  solution: { from: string; to: string; promotion?: string }[];
  hint?: string;
}

// Mock puzzle data with FENs
const MOCK_PUZZLES: Puzzle[] = [
  {
    id: 1,
    fen: "r1bqkbnr/pppp1ppp/2n5/1B2p3/4P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 0 4",
    title: "Fork Attack",
    difficulty: "easy",
    description: "Find the knight fork that wins material",
    solution: [{ from: "f3", to: "g5" }],
    hint: "Look for a knight move that attacks two pieces"
  },
  {
    id: 2,
    fen: "rnbqkb1r/pppp1ppp/5n2/2B1p3/4P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 0 4",
    title: "Pin and Win",
    difficulty: "medium",
    description: "Use a pin to create a winning advantage",
    solution: [{ from: "c4", to: "f7" }],
    hint: "The bishop can pin the knight to the king"
  },
  {
    id: 3,
    fen: "r1bqk2r/pppp1ppp/2n2n2/2B1p3/4P3/3N1N2/PPPP1PPP/R1BQK2R w KQkq - 0 6",
    title: "Discovered Attack",
    difficulty: "hard",
    description: "Execute a discovered attack for checkmate",
    solution: [{ from: "d3", to: "e5" }, { from: "c4", to: "f7" }],
    hint: "Move the knight first to reveal the bishop's attack"
  },
  {
    id: 4,
    fen: "rnbqkbnr/pp1ppppp/2p5/3p4/3P4/2N5/PP1PPPPP/R1BQKBNR w KQkq - 0 3",
    title: "Center Control",
    difficulty: "easy",
    description: "Control the center with your knight",
    solution: [{ from: "c3", to: "d5" }],
    hint: "Knights are excellent in the center"
  },
  {
    id: 5,
    fen: "rnbqk2r/pppp1ppp/5n2/2B1p3/3PP3/5N2/PPP2PPP/RNBQK2R w KQkq - 0 5",
    title: "Double Attack",
    difficulty: "medium",
    description: "Create a double attack with your bishop",
    solution: [{ from: "c4", to: "e6" }],
    hint: "Look for squares that attack multiple pieces"
  }
];

export default function PuzzlesPage() {
  const [selectedPuzzle, setSelectedPuzzle] = useState<Puzzle | null>(null);
  const [completedPuzzles, setCompletedPuzzles] = useState<Set<number>>(new Set());
  const [currentMove, setCurrentMove] = useState(0);
  const [showReward, setShowReward] = useState(false);
  const [rewardAmount, setRewardAmount] = useState(0);
  const [isCorrect, setIsCorrect] = useState<boolean | null>(null);
  const [showHint, setShowHint] = useState(false);
  const [isClaiming, setIsClaiming] = useState(false);

  const { address, status: walletStatus } = useAppContext();
  const { addToast } = useToast();

  const { execute: executeClaim } = useTrackedTransaction({
    type: "claim",
    label: `Claim ${rewardAmount || "0.01"} XLM reward`,
    amount: String(rewardAmount),
  });

  const getDifficultyColor = (difficulty: string) => {
    switch (difficulty) {
      case "easy": return "text-green-400 bg-green-400/20 border-green-400/30";
      case "medium": return "text-yellow-400 bg-yellow-400/20 border-yellow-400/30";
      case "hard": return "text-red-400 bg-red-400/20 border-red-400/30";
      default: return "text-gray-400 bg-gray-400/20 border-gray-400/30";
    }
  };

  const getDifficultyIcon = (difficulty: string) => {
    switch (difficulty) {
      case "easy": return <FaStar className="text-green-400" />;
      case "medium": return <FaStar className="text-yellow-400" />;
      case "hard": return <FaStar className="text-red-400" />;
      default: return <FaStar className="text-gray-400" />;
    }
  };

  const handlePuzzleSelect = (puzzle: Puzzle) => {
    setSelectedPuzzle(puzzle);
    setCurrentMove(0);
    setIsCorrect(null);
    setShowHint(false);
  };

  const handleSolutionSubmit = useCallback(() => {
    if (!selectedPuzzle) return;

    // Simulate solution validation
    const isSolutionCorrect = currentMove === selectedPuzzle.solution.length - 1;
    setIsCorrect(isSolutionCorrect);

    if (isSolutionCorrect) {
      // Mark puzzle as completed
      setCompletedPuzzles((prev: Set<number>) => new Set([...prev, selectedPuzzle.id]));

      // Simulate backend verification and reward
      const reward = 0.01; // 0.01 XLM reward
      setRewardAmount(reward);
      setShowReward(true);

      addToast({
        severity: "success",
        title: "Puzzle Complete!",
        detail: `You earned ${reward} XLM. Click claim to receive your reward.`,
      });

      // Hide reward popup after 4 seconds
      setTimeout(() => {
        setShowReward(false);
      }, 4000);
    }
  }, [selectedPuzzle, currentMove, addToast]);

  const handleMoveNext = () => {
    if (!selectedPuzzle) return;
    if (currentMove < selectedPuzzle.solution.length - 1) {
      setCurrentMove(currentMove + 1);
    } else {
      handleSolutionSubmit();
    }
  };

  const handleMoveBack = () => {
    if (currentMove > 0) {
      setCurrentMove(currentMove - 1);
      setIsCorrect(null);
    }
  };

  const handleReset = () => {
    setCurrentMove(0);
    setIsCorrect(null);
    setShowHint(false);
  };

  const handleClaimReward = useCallback(async () => {
    if (walletStatus !== "connected" || !address) {
      addToast({
        severity: "warning",
        title: "Wallet Required",
        detail: "Connect your Freighter wallet to claim XLM rewards.",
      });
      return;
    }
    setIsClaiming(true);

    const result = await executeClaim(async (txId: string) => {
      // Simulate on-chain reward claim — in production this would invoke a Soroban contract
      void txId;
      await new Promise((resolve) => setTimeout(resolve, 1500));
      return { hash: undefined };
    });

    setIsClaiming(false);
    if (result !== undefined) {
      setShowReward(false);
    }
  }, [walletStatus, address, addToast, executeClaim]);

  const completionRate = Math.round((completedPuzzles.size / MOCK_PUZZLES.length) * 100);

  // Chess game instance for the selected puzzle
  const [puzzleGame] = useState(() => new Chess());
  const [puzzleFen, setPuzzleFen] = useState("");

  // Initialize puzzle board when puzzle is selected
  React.useEffect(() => {
    if (selectedPuzzle) {
      puzzleGame.load(selectedPuzzle.fen);
      setPuzzleFen(puzzleGame.fen());
    }
  }, [selectedPuzzle, puzzleGame]);

  const handlePuzzleMove = useCallback(
    ({ sourceSquare, targetSquare }: { sourceSquare: string; targetSquare: string }) => {
      if (!selectedPuzzle) return false;
      try {
        const move = puzzleGame.move({
          from: sourceSquare,
          to: targetSquare,
          promotion: "q",
        });
        if (move) {
          setPuzzleFen(puzzleGame.fen());
          // Check if this matches the expected solution move
          const expectedMove = selectedPuzzle.solution[currentMove];
          if (expectedMove && sourceSquare === expectedMove.from && targetSquare === expectedMove.to) {
            if (currentMove < selectedPuzzle.solution.length - 1) {
              setCurrentMove((prev: number) => prev + 1);
            } else {
              handleSolutionSubmit();
            }
          }
          return true;
        }
      } catch {
        // invalid move
      }
      return false;
    },
    [selectedPuzzle, puzzleGame, currentMove, handleSolutionSubmit],
  );

  if (selectedPuzzle) {
    return (
      <div className="min-h-screen p-4 md:p-8" role="region" aria-label="Chess Puzzles">
        {/* Reward Popup */}
        {showReward && (
          <div className="fixed inset-0 bg-black/60 backdrop-blur-sm z-50 flex items-center justify-center animate-overlay-in" role="dialog" aria-modal="true" aria-label="Puzzle reward">
            <div className="bg-gray-900 p-8 rounded-2xl border border-emerald-500/30 text-center animate-modal-in max-w-sm w-full mx-4">
              <div className="flex flex-col items-center gap-4">
                <div className="w-16 h-16 rounded-full bg-gradient-to-br from-yellow-400/20 to-emerald-500/20 flex items-center justify-center">
                  <FaTrophy className="text-4xl text-yellow-400" />
                </div>
                <h3 className="text-2xl font-bold text-emerald-400">Puzzle Complete!</h3>
                <p className="text-xl text-white">+{rewardAmount} XLM</p>
                <div className="text-sm text-gray-400 space-y-1">
                  <p>✅ Backend verification complete</p>
                  <p>🎁 Reward ready to claim</p>
                </div>
                <button
                  onClick={handleClaimReward}
                  disabled={isClaiming}
                  className="w-full py-3 rounded-xl bg-gradient-to-r from-emerald-500 to-teal-600 hover:from-emerald-600 hover:to-teal-700 disabled:opacity-50 text-white font-bold text-sm transition-all duration-300 hover:scale-[1.02] active:scale-[0.98]"
                >
                  {isClaiming ? (
                    <span className="flex items-center justify-center gap-2">
                      <svg className="animate-spin h-4 w-4" viewBox="0 0 24 24">
                        <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" fill="none" />
                        <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
                      </svg>
                      Claiming...
                    </span>
                  ) : walletStatus === "connected" ? (
                    "Claim Reward"
                  ) : (
                    "Connect Wallet to Claim"
                  )}
                </button>
              </div>
            </div>
          </div>
        )}

        {/* Header */}
        <div className="max-w-4xl mx-auto">
          {/* Top bar */}
          <div className="flex items-center justify-between mb-4">
            <button
              onClick={() => setSelectedPuzzle(null)}
              aria-label="Back to puzzles list"
              className="flex items-center gap-2 text-gray-400 hover:text-white transition-colors text-sm"
            >
              <FaArrowLeft />
              <span>Back to Puzzles</span>
            </button>
            <Web3StatusBar />
          </div>
          
          {/* Puzzle Info */}
          <div className="bg-gray-800/60 p-5 rounded-xl border border-gray-700/50 mb-4 animate-slide-up">
            <div className="flex items-center justify-between mb-3">
              <h2 className="text-xl font-bold text-white">{selectedPuzzle.title}</h2>
              <div className={`flex items-center gap-2 px-3 py-1 rounded-full border ${getDifficultyColor(selectedPuzzle.difficulty)}`}>
                {getDifficultyIcon(selectedPuzzle.difficulty)}
                <span className="text-sm font-medium capitalize">{selectedPuzzle.difficulty}</span>
              </div>
            </div>
            <p className="text-gray-300 text-sm mb-4">{selectedPuzzle.description}</p>
                    
            {/* Progress */}
            <div className="flex items-center gap-4 text-sm text-gray-400">
              <span>Move {currentMove + 1} of {selectedPuzzle.solution.length}</span>
              <div
                className="flex-1 bg-gray-700 rounded-full h-2"
                role="progressbar"
                aria-valuenow={currentMove + 1}
                aria-valuemin={0}
                aria-valuemax={selectedPuzzle.solution.length}
                aria-label="Puzzle progress"
              >
                <div 
                  className="bg-gradient-to-r from-teal-500 to-blue-500 h-2 rounded-full transition-all duration-500"
                  style={{ width: `${((currentMove + 1) / selectedPuzzle.solution.length) * 100}%` }}
                />
              </div>
            </div>
          </div>
          
          {/* Real Chess Board */}
          <div className="flex flex-col lg:flex-row gap-6 items-center lg:items-start justify-center">
            <div className="w-full max-w-[500px]">
              <ChessboardComponent position={puzzleFen} onDrop={handlePuzzleMove} />
            </div>
          
            {/* Controls */}
            <div className="w-full lg:w-72 bg-gray-800/60 p-5 rounded-xl border border-gray-700/50">
              <div className="flex flex-col gap-3">
                <div className="flex gap-2">
                  <button
                    onClick={handleMoveBack}
                    disabled={currentMove === 0}
                    className="flex-1 px-4 py-2.5 bg-gray-700/60 hover:bg-gray-600/60 disabled:bg-gray-800/40 disabled:text-gray-600 rounded-xl text-white font-medium transition-colors text-sm"
                  >
                    <FaTimes className="inline mr-2" />
                    Back
                  </button>
                  <button
                    onClick={handleReset}
                    className="flex-1 px-4 py-2.5 bg-gray-700/60 hover:bg-gray-600/60 rounded-xl text-white font-medium transition-colors text-sm"
                  >
                    <FaRedo className="inline mr-2" />
                    Reset
                  </button>
                </div>
                <button
                  onClick={() => setShowHint(!showHint)}
                  aria-pressed={showHint}
                  className="w-full px-4 py-2.5 bg-blue-500/10 hover:bg-blue-500/20 border border-blue-500/30 rounded-xl text-blue-400 font-medium transition-colors text-sm"
                >
                  💡 Hint
                </button>
                <button
                  onClick={handleMoveNext}
                  className="w-full px-6 py-3 bg-gradient-to-r from-emerald-500 to-teal-600 hover:from-emerald-600 hover:to-teal-700 rounded-xl text-white font-bold text-sm transition-all duration-300 hover:scale-[1.02] active:scale-[0.98]"
                >
                  {currentMove === selectedPuzzle.solution.length - 1 ? (
                    <>
                      <FaCheck className="inline mr-2" />
                      Submit Solution
                    </>
                  ) : (
                    "Next Move →"
                  )}
                </button>
              </div>
          
              {/* Hint Display */}
              {showHint && selectedPuzzle.hint && (
                <div className="mt-3 p-3 bg-blue-500/10 border border-blue-500/30 rounded-lg animate-scale-in">
                  <p className="text-blue-400 text-sm">
                    💡 {selectedPuzzle.hint}
                  </p>
                </div>
              )}
          
              {/* Result Display */}
              {isCorrect !== null && (
                <div
                  className={`mt-3 p-3 rounded-lg border animate-scale-in ${
                    isCorrect
                      ? 'bg-emerald-500/10 border-emerald-500/30 text-emerald-400'
                      : 'bg-red-500/10 border-red-500/30 text-red-400'
                  }`}
                  role="alert"
                  aria-live="assertive"
                >
                  <p className="font-medium text-sm">
                    {isCorrect ? '✅ Correct! Well done!' : '❌ Not quite right. Try again!'}
                  </p>
                </div>
              )}
            </div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen p-4 md:p-8">
      <div className="max-w-6xl mx-auto">
        {/* Header */}
        <div className="text-center mb-8 animate-fade-in">
          <h1 className="text-4xl font-bold text-white mb-4">
            <FaTrophy className="inline mr-3 text-yellow-400" />
            Learn-to-Earn Puzzles
          </h1>
          <p className="text-xl text-gray-300 mb-6">
            Solve chess puzzles and earn micro XLM rewards!
          </p>
          
          {/* Stats */}
          <div className="flex justify-center gap-6 mb-8">
            <div className="bg-gray-800/60 px-6 py-4 rounded-xl border border-gray-700/50 animate-scale-in" style={{ animationDelay: "0.1s" }}>
              <p className="text-2xl font-bold text-white">{completedPuzzles.size}</p>
              <p className="text-sm text-gray-400">Completed</p>
            </div>
            <div className="bg-gray-800/60 px-6 py-4 rounded-xl border border-gray-700/50 animate-scale-in" style={{ animationDelay: "0.2s" }}>
              <p className="text-2xl font-bold text-white">{completionRate}%</p>
              <p className="text-sm text-gray-400">Completion Rate</p>
            </div>
            <div className="bg-gray-800/60 px-6 py-4 rounded-xl border border-gray-700/50 animate-scale-in" style={{ animationDelay: "0.3s" }}>
              <p className="text-2xl font-bold text-emerald-400">{(completedPuzzles.size * 0.01).toFixed(2)}</p>
              <p className="text-sm text-gray-400">XLM Earned</p>
            </div>
          </div>
        </div>

        {/* Puzzle Grid */}
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
          {MOCK_PUZZLES.map((puzzle, idx) => {
            const isCompleted = completedPuzzles.has(puzzle.id);
            return (
              <div
                key={puzzle.id}
                className={`bg-gray-800/60 p-6 rounded-xl border transition-all duration-300 hover:scale-[1.03] cursor-pointer animate-slide-up ${
                  isCompleted 
                    ? 'border-emerald-500/30 bg-emerald-500/5' 
                    : 'border-gray-700/50 hover:border-gray-600/50'
                }`}
                style={{ animationDelay: `${idx * 0.05}s` }}
                onClick={() => handlePuzzleSelect(puzzle)}
                onKeyDown={(e: React.KeyboardEvent) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); handlePuzzleSelect(puzzle); } }}
                role="button"
                tabIndex={0}
                aria-label={`${puzzle.title} — ${puzzle.difficulty} difficulty${isCompleted ? ', completed' : ''}`}
              >
                <div className="flex items-center justify-between mb-4">
                  <h3 className="text-lg font-bold text-white">{puzzle.title}</h3>
                  {isCompleted && <FaCheck className="text-emerald-400 text-xl" />}
                </div>
                
                <div className={`inline-flex items-center gap-2 px-3 py-1 rounded-full border text-sm mb-3 ${getDifficultyColor(puzzle.difficulty)}`}>
                  {getDifficultyIcon(puzzle.difficulty)}
                  <span className="capitalize">{puzzle.difficulty}</span>
                </div>
                
                <p className="text-gray-300 text-sm mb-4">{puzzle.description}</p>
                
                <div className="flex items-center justify-between">
                  <span className="text-xs text-gray-500">Puzzle #{puzzle.id}</span>
                  <span className="text-xs text-emerald-400 font-medium">+0.01 XLM</span>
                </div>
              </div>
            );
          })}
        </div>

        {/* Instructions */}
        <div className="mt-12 bg-gray-800/60 p-6 rounded-xl border border-gray-700/50">
          <h3 className="text-xl font-bold text-white mb-4">How to Play</h3>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-6 text-gray-300">
            <div>
              <h4 className="font-semibold text-white mb-2">1. Select a Puzzle</h4>
              <p className="text-sm">Choose from puzzles of varying difficulty levels</p>
            </div>
            <div>
              <h4 className="font-semibold text-white mb-2">2. Find the Solution</h4>
              <p className="text-sm">Navigate through the correct moves to solve the puzzle</p>
            </div>
            <div>
              <h4 className="font-semibold text-white mb-2">3. Earn Rewards</h4>
              <p className="text-sm">Complete puzzles to earn 0.01 XLM each</p>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
