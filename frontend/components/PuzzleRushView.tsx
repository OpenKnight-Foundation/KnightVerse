"use client";

import dynamic from "next/dynamic";
import { useEffect, useRef, useState } from "react";
import { ArrowLeft, Flame, Play, RotateCcw, Trophy, X } from "lucide-react";
import { Chess } from "chess.js";

const ChessboardComponent = dynamic(() => import("@/components/chess/ChessboardComponent"), { ssr: false });
type RushMode = "three" | "five" | "survival";
type RushPuzzle = { id: string; fen: string };
type PuzzleRushViewProps = { onExit: () => void; onVerifyMove?: (puzzleId: string, move: { from: string; to: string }) => Promise<boolean> };
const puzzles: RushPuzzle[] = [
  { id: "rush-1", fen: "r1bqkbnr/pppp1ppp/2n5/1B2p3/4P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 0 4" },
  { id: "rush-2", fen: "rnbqkb1r/pppp1ppp/5n2/2B1p3/4P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 0 4" },
  { id: "rush-3", fen: "rnbqkbnr/pp1ppppp/2p5/3p4/3P4/2N5/PP1PPPPP/R1BQKBNR w KQkq - 0 3" },
];
const durations: Record<RushMode, number> = { three: 180, five: 300, survival: 0 };

export default function PuzzleRushView({ onExit, onVerifyMove }: PuzzleRushViewProps) {
  const [mode, setMode] = useState<RushMode | null>(null);
  const [timeLeft, setTimeLeft] = useState(0);
  const [score, setScore] = useState(0);
  const [streak, setStreak] = useState(0);
  const [strikes, setStrikes] = useState(0);
  const [puzzleIndex, setPuzzleIndex] = useState(0);
  const [fen, setFen] = useState("");
  const [feedback, setFeedback] = useState<"correct" | "incorrect" | null>(null);
  const [finished, setFinished] = useState(false);
  const gameRef = useRef(new Chess());
  const currentPuzzle = puzzles[puzzleIndex % puzzles.length];

  const start = (nextMode: RushMode) => { setMode(nextMode); setTimeLeft(durations[nextMode]); setScore(0); setStreak(0); setStrikes(0); setPuzzleIndex(0); setFinished(false); gameRef.current.load(puzzles[0].fen); setFen(gameRef.current.fen()); };
  useEffect(() => { if (!mode || finished || mode === "survival") return; const timer = window.setInterval(() => setTimeLeft((value) => { if (value <= 1) { setFinished(true); return 0; } return value - 1; }), 1000); return () => window.clearInterval(timer); }, [mode, finished]);
  const finishIfNeeded = (nextStrikes: number) => { if (mode === "survival" && nextStrikes >= 3) setFinished(true); };
  const playTone = (frequency: number) => { if (typeof window === "undefined") return; const context = new AudioContext(); const oscillator = context.createOscillator(); const gain = context.createGain(); oscillator.frequency.value = frequency; gain.gain.value = 0.04; oscillator.connect(gain); gain.connect(context.destination); oscillator.start(); oscillator.stop(context.currentTime + 0.12); };
  const handleMove = async ({ sourceSquare, targetSquare }: { sourceSquare: string; targetSquare: string }) => {
    if (!mode || finished) return false;
    try { const move = gameRef.current.move({ from: sourceSquare, to: targetSquare, promotion: "q" }); if (!move) return false; const correct = onVerifyMove ? await onVerifyMove(currentPuzzle.id, { from: sourceSquare, to: targetSquare }) : true; if (correct) { const nextStreak = streak + 1; setStreak(nextStreak); setScore((value) => value + 100 * Math.max(1, nextStreak)); setFeedback("correct"); playTone(880); window.setTimeout(() => { const next = (puzzleIndex + 1) % puzzles.length; setPuzzleIndex(next); gameRef.current.load(puzzles[next].fen); setFen(gameRef.current.fen()); setFeedback(null); }, 120); } else { const nextStrikes = strikes + 1; setStrikes(nextStrikes); setStreak(0); setFeedback("incorrect"); playTone(160); finishIfNeeded(nextStrikes); gameRef.current.load(currentPuzzle.fen); setFen(gameRef.current.fen()); } return true; } catch { return false; }
  };
  const formatTime = (value: number) => `${Math.floor(value / 60)}:${String(value % 60).padStart(2, "0")}`;

  if (!mode) return <main className="min-h-screen bg-slate-950 px-4 py-12 text-white"><div className="mx-auto max-w-4xl"><button onClick={onExit} className="mb-10 flex items-center gap-2 text-sm text-slate-400 hover:text-white"><ArrowLeft size={16} />Back to puzzles</button><div className="mb-10 max-w-2xl"><p className="mb-3 text-sm uppercase tracking-[0.25em] text-orange-300">Arcade training</p><h1 className="text-5xl font-bold tracking-tight">Puzzle Rush</h1><p className="mt-4 text-slate-400">Solve continuously. Build a streak. Three strikes ends survival.</p></div><div className="grid gap-4 md:grid-cols-3">{(["three", "five", "survival"] as RushMode[]).map((item) => <button key={item} onClick={() => start(item)} className="border border-slate-700 bg-slate-900 p-6 text-left transition hover:-translate-y-1 hover:border-orange-300"><p className="text-lg font-bold">{item === "survival" ? "Survival" : `${item === "three" ? "3" : "5"}-Minute`}</p><p className="mt-2 text-sm text-slate-400">{item === "survival" ? "Three strikes" : "Race the clock"}</p><span className="mt-8 flex items-center gap-2 text-sm text-orange-300"><Play size={15} />Start run</span></button>)}</div></div></main>;
  if (finished) return <main className="min-h-screen bg-slate-950 px-4 py-12 text-white"><div className="mx-auto max-w-xl border border-slate-700 bg-slate-900 p-8 text-center"><Trophy className="mx-auto mb-4 text-yellow-300" size={42} /><p className="text-sm uppercase tracking-[0.2em] text-orange-300">Run complete</p><h1 className="mt-2 text-4xl font-bold">Final score {score.toLocaleString()}</h1><p className="mt-3 text-slate-400">{puzzleIndex} puzzles solved with a best streak of {streak}.</p><div className="mt-8 flex gap-3"><button onClick={() => setMode(null)} className="flex-1 border border-slate-600 px-4 py-3 text-sm">Choose mode</button><button onClick={() => start(mode)} className="flex-1 bg-orange-300 px-4 py-3 text-sm font-bold text-slate-950"><RotateCcw className="mr-2 inline" size={16} />Replay</button></div></div></main>;
  return <main className="min-h-screen bg-slate-950 px-4 py-8 text-white"><div className="mx-auto max-w-6xl"><div className="mb-6 flex items-center justify-between"><button onClick={onExit} aria-label="Exit Puzzle Rush" className="p-2 text-slate-400 hover:text-white"><X size={20} /></button><div className="flex items-center gap-6 text-sm"><span>Score <strong className="text-orange-300">{score.toLocaleString()}</strong></span><span className="flex items-center gap-1"><Flame size={16} className="text-orange-300" />{streak}x</span><span>{mode === "survival" ? `${3 - strikes} strikes` : formatTime(timeLeft)}</span></div></div><div className="mb-6 h-2 bg-slate-800">{mode !== "survival" && <div className="h-full bg-orange-300 transition-[width]" style={{ width: `${(timeLeft / durations[mode]) * 100}%` }} />}</div><div className="mx-auto max-w-[560px]"><ChessboardComponent position={fen} onDrop={handleMove} /><p className={`mt-4 min-h-6 text-center text-sm ${feedback === "correct" ? "text-emerald-300" : "text-red-300"}`}>{feedback === "correct" ? "Correct. Next position..." : feedback === "incorrect" ? "Incorrect move. The position has been reset." : "Find the best move"}</p></div></div></main>;
}