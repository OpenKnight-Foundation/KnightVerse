"use client";

import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import dynamic from "next/dynamic";
import { Chess } from "chess.js";
import { useToast } from "@/components/ui/toast";
import { OpeningExplorer } from "@/components/openings/OpeningExplorer";
import {
  ECO_BOOK,
  classifyOpening,
  statsForEco,
  type OpeningStats,
} from "@/lib/openings";
import { getOpeningStats } from "@/lib/openingStatsService";
import {
  createLine,
  exportLinesToPgn,
  importPgnToLines,
  loadRepertoire,
  saveRepertoire,
  syncRepertoireToProfile,
  type RepertoireColor,
  type RepertoireLine,
  type RepertoireState,
} from "@/lib/repertoireStore";
import {
  dueCards,
  qualityFromDrillResult,
  sm2Review,
} from "@/lib/spacedRepetition";

const ChessboardComponent = dynamic(
  () => import("@/components/chess/ChessboardComponent"),
  { ssr: false },
);

const START_FEN = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
type Mode = "explore" | "build" | "drill";

function fenAfterMoves(moves: string[]): string {
  const c = new Chess();
  for (const san of moves) {
    try {
      c.move(san);
    } catch {
      break;
    }
  }
  return c.fen();
}

export default function OpeningsPage() {
  const { addToast } = useToast();
  const [mode, setMode] = useState<Mode>("explore");
  const [color, setColor] = useState<RepertoireColor>("white");
  const [rep, setRep] = useState<RepertoireState>({ white: [], black: [] });
  const [selectedEco, setSelectedEco] = useState<string | null>(ECO_BOOK[0].eco);
  const [boardMoves, setBoardMoves] = useState<string[]>([]);
  const [boardFen, setBoardFen] = useState(START_FEN);
  const [liveStats, setLiveStats] = useState<Record<string, OpeningStats>>({});
  const [drillIndex, setDrillIndex] = useState(0);
  const [drillFen, setDrillFen] = useState(START_FEN);
  const [drillMessage, setDrillMessage] = useState<string | null>(null);
  const [drillAttempts, setDrillAttempts] = useState(0);
  const [drillHints, setDrillHints] = useState(0);
  const [lineName, setLineName] = useState("");
  const gameRef = useRef(new Chess());
  const fileRef = useRef<HTMLInputElement>(null);

  // Load repertoire once (localStorage-first).
  useEffect(() => {
    setRep(loadRepertoire());
  }, []);

  useEffect(() => {
    saveRepertoire(rep);
  }, [rep]);

  const lines = rep[color];

  const statsByEco = useMemo(() => {
    const out: Record<string, OpeningStats> = {};
    for (const o of ECO_BOOK) out[o.eco] = liveStats[o.eco] ?? statsForEco(o.eco);
    return out;
  }, [liveStats]);

  const depthByEco = useMemo(() => {
    const out: Record<string, number> = {};
    for (const o of ECO_BOOK) out[o.eco] = o.moves.length;
    for (const l of [...rep.white, ...rep.black]) {
      out[l.eco] = Math.max(out[l.eco] ?? 0, l.moves.length);
    }
    return out;
  }, [rep]);

  const frequencyByEco = useMemo(() => {
    const out: Record<string, number> = {};
    for (const l of [...rep.white, ...rep.black]) {
      out[l.eco] = (out[l.eco] ?? 0) + 1;
    }
    return out;
  }, [rep]);

  const liveClassification = classifyOpening(boardMoves);

  // Fetch live master stats for the selected + current board line.
  useEffect(() => {
    let cancelled = false;
    async function load() {
      const targets = ECO_BOOK.filter(
        (o) => o.eco === selectedEco || o.eco === liveClassification?.eco,
      );
      for (const t of targets) {
        const fen = fenAfterMoves(t.moves);
        const s = await getOpeningStats(fen, t.eco);
        if (!cancelled) {
          setLiveStats((prev) => ({
            ...prev,
            [t.eco]: { white: s.white, draws: s.draws, black: s.black, games: s.games },
          }));
        }
      }
    }
    void load();
    return () => {
      cancelled = true;
    };
  }, [selectedEco, liveClassification?.eco]);

  const resetBoard = useCallback(() => {
    gameRef.current = new Chess();
    setBoardMoves([]);
    setBoardFen(gameRef.current.fen());
  }, []);

  const handleBoardDrop = useCallback(
    ({ sourceSquare, targetSquare }: { sourceSquare: string; targetSquare: string }) => {
      try {
        const move = gameRef.current.move({ from: sourceSquare, to: targetSquare, promotion: "q" });
        if (!move) return false;
        setBoardMoves((m) => [...m, move.san]);
        setBoardFen(gameRef.current.fen());
        return true;
      } catch {
        return false;
      }
    },
    [],
  );

  const handleSaveLine = useCallback(() => {
    if (boardMoves.length === 0) {
      addToast({ severity: "warning", title: "No moves", detail: "Play moves on the board first." });
      return;
    }
    const classified = classifyOpening(boardMoves);
    const line = createLine({
      name: lineName.trim() || classified?.name || `${color} line ${lines.length + 1}`,
      eco: classified?.eco || "A00",
      moves: boardMoves,
    });
    setRep((prev) => ({ ...prev, [color]: [...prev[color], line] }));
    void syncRepertoireToProfile({ ...rep, [color]: [...rep[color], line] });
    setLineName("");
    resetBoard();
    addToast({ severity: "success", title: "Line saved", detail: `${line.name} added to ${color} repertoire.` });
  }, [boardMoves, lineName, color, lines.length, rep, resetBoard, addToast]);

  const handleDeleteLine = useCallback(
    (id: string) => {
      setRep((prev) => ({ ...prev, [color]: prev[color].filter((l) => l.id !== id) }));
    },
    [color],
  );

  const handleExport = useCallback(() => {
    const pgn = exportLinesToPgn(lines, color);
    if (!pgn) {
      addToast({ severity: "warning", title: "Nothing to export", detail: "Your repertoire is empty." });
      return;
    }
    const blob = new Blob([pgn], { type: "application/x-chess-pgn" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `knightverse-${color}-repertoire.pgn`;
    a.click();
    URL.revokeObjectURL(url);
  }, [lines, color, addToast]);

  const handleImportFile = useCallback(
    async (file: File) => {
      const text = await file.text();
      const imported = importPgnToLines(text);
      if (imported.length === 0) {
        addToast({ severity: "error", title: "Import failed", detail: "No valid PGN games found." });
        return;
      }
      const newLines: RepertoireLine[] = imported.map((moves, i) => {
        const classified = classifyOpening(moves);
        return createLine({
          name: classified?.name || `Imported line ${lines.length + i + 1}`,
          eco: classified?.eco || "A00",
          moves,
        });
      });
      setRep((prev) => ({ ...prev, [color]: [...prev[color], ...newLines] }));
      addToast({ severity: "success", title: "PGN imported", detail: `${newLines.length} line(s) added.` });
    },
    [color, lines.length, addToast],
  );

  // ── Drill mode ──────────────────────────────────────────────
  const dueLines = useMemo(() => dueCards(lines), [lines]);
  const drillQueue = dueLines.length > 0 ? dueLines : lines;
  const activeDrill = drillQueue[drillIndex % Math.max(1, drillQueue.length)] ?? null;

  useEffect(() => {
    if (mode !== "drill" || !activeDrill) return;
    // Present position one move before the repertoire move (opponent to move lead-in).
    const leadIn = activeDrill.moves.slice(0, Math.max(0, activeDrill.moves.length - 1));
    const c = new Chess();
    for (const san of leadIn) {
      try {
        c.move(san);
      } catch {
        break;
      }
    }
    setDrillFen(c.fen());
    setDrillMessage(null);
    setDrillAttempts(0);
    setDrillHints(0);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mode, drillIndex, activeDrill?.id]);

  const expectedDrillSan = activeDrill?.moves[activeDrill.moves.length - 1] ?? null;

  const gradeDrill = useCallback(
    (correct: boolean) => {
      if (!activeDrill) return;
      const quality = qualityFromDrillResult({
        correct,
        hintsUsed: drillHints,
        attempts: drillAttempts + 1,
      });
      const next = sm2Review(activeDrill.srs, quality);
      setRep((prev) => ({
        ...prev,
        [color]: prev[color].map((l) =>
          l.id === activeDrill.id ? { ...l, srs: next, updatedAt: Date.now() } : l,
        ),
      }));
      if (correct) {
        setDrillMessage(`Correct! Next review in ${next.interval} day(s).`);
        setTimeout(() => setDrillIndex((i) => i + 1), 900);
      } else {
        setDrillMessage(
          drillAttempts + 1 >= 3 && expectedDrillSan
            ? `The move was ${expectedDrillSan}. It will come back tomorrow.`
            : "Not quite — try again.",
        );
        setDrillAttempts((a) => a + 1);
      }
    },
    [activeDrill, drillAttempts, drillHints, color, expectedDrillSan],
  );

  const handleDrillDrop = useCallback(
    ({ sourceSquare, targetSquare }: { sourceSquare: string; targetSquare: string }) => {
      if (!activeDrill || !expectedDrillSan) return false;
      const c = new Chess(drillFen);
      let playedSan: string | null = null;
      try {
        const m = c.move({ from: sourceSquare, to: targetSquare, promotion: "q" });
        if (!m) return false;
        playedSan = m.san.replace(/[+#!?]+$/g, "");
      } catch {
        return false;
      }
      const expected = expectedDrillSan.replace(/[+#!?]+$/g, "");
      const correct = playedSan === expected;
      if (correct) setDrillFen(c.fen());
      gradeDrill(correct);
      return correct;
    },
    [activeDrill, drillFen, expectedDrillSan, gradeDrill],
  );

  const selectedOpening = ECO_BOOK.find((o) => o.eco === selectedEco) ?? null;

  return (
    <div className="min-h-screen p-4 md:p-8" role="region" aria-label="Opening repertoire explorer">
      <div className="max-w-7xl mx-auto">
        <div className="text-center mb-6">
          <h1 className="text-3xl md:text-4xl font-bold text-white">Opening Repertoire Explorer</h1>
          <p className="text-gray-300 mt-2">
            Study ECO lines, build White/Black repertoires, and drill them with spaced repetition.
          </p>
          <div className="flex flex-wrap justify-center gap-2 mt-4" role="tablist" aria-label="Trainer modes">
            {(["explore", "build", "drill"] as Mode[]).map((m) => (
              <button
                key={m}
                role="tab"
                aria-selected={mode === m}
                onClick={() => setMode(m)}
                className={`px-4 py-2 rounded-xl text-sm font-semibold border transition-colors ${
                  mode === m
                    ? "bg-emerald-500/20 border-emerald-500/50 text-emerald-300"
                    : "bg-gray-800/60 border-gray-700/50 text-gray-300 hover:text-white"
                }`}
              >
                {m === "explore" ? "Explore" : m === "build" ? "Build Repertoire" : `Drill (${drillQueue.length} due)`}
              </button>
            ))}
            <div className="flex rounded-xl overflow-hidden border border-gray-700/50" role="group" aria-label="Repertoire color">
              {(["white", "black"] as RepertoireColor[]).map((c) => (
                <button
                  key={c}
                  onClick={() => setColor(c)}
                  aria-pressed={color === c}
                  className={`px-4 py-2 text-sm font-semibold capitalize ${
                    color === c ? "bg-teal-500 text-slate-950" : "bg-gray-800/60 text-gray-300"
                  }`}
                >
                  {c}
                </button>
              ))}
            </div>
          </div>
        </div>

        <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
          <OpeningExplorer
            selectedEco={selectedEco}
            onSelect={setSelectedEco}
            statsByEco={statsByEco}
            depthByEco={depthByEco}
            frequencyByEco={frequencyByEco}
          />

          <div className="bg-gray-800/60 rounded-xl border border-gray-700/50 p-4">
            <h2 className="text-lg font-bold text-white mb-2">
              {mode === "drill" ? "Drill Board" : "Repertoire Board"}
            </h2>
            <p className="text-xs text-gray-400 mb-3" aria-live="polite">
              {mode === "drill"
                ? activeDrill
                  ? `Play the ${color} move for: ${activeDrill.name}${drillHints > 0 && expectedDrillSan ? ` (hint: ${expectedDrillSan})` : ""}`
                  : "No lines yet — build or import a repertoire first."
                : liveClassification
                  ? `${liveClassification.eco} · ${liveClassification.name}`
                  : "Play moves — ECO classification appears here."}
            </p>
            <ChessboardComponent
              position={mode === "drill" ? drillFen : boardFen}
              onDrop={mode === "drill" ? handleDrillDrop : handleBoardDrop}
              orientation={color === "black" ? "black" : "white"}
            />
            {mode === "drill" ? (
              <div className="mt-3 space-y-2">
                {drillMessage && (
                  <p className="text-sm text-emerald-300 bg-emerald-500/10 border border-emerald-500/30 rounded-lg p-2" role="status">
                    {drillMessage}
                  </p>
                )}
                <div className="flex gap-2">
                  <button
                    onClick={() => {
                      setDrillHints((h) => h + 1);
                      setDrillMessage(expectedDrillSan ? `Hint: play ${expectedDrillSan}` : "No hint available.");
                    }}
                    className="flex-1 px-3 py-2 text-sm rounded-xl bg-blue-500/10 border border-blue-500/30 text-blue-300 hover:bg-blue-500/20"
                  >
                    Hint
                  </button>
                  <button
                    onClick={() => gradeDrill(false)}
                    className="flex-1 px-3 py-2 text-sm rounded-xl bg-gray-700/60 text-gray-200 hover:bg-gray-600/60"
                  >
                    I forgot
                  </button>
                  <button
                    onClick={() => setDrillIndex((i) => i + 1)}
                    className="flex-1 px-3 py-2 text-sm rounded-xl bg-gray-700/60 text-gray-200 hover:bg-gray-600/60"
                  >
                    Skip
                  </button>
                </div>
                {activeDrill && (
                  <p className="text-[11px] text-gray-500">
                    Interval {activeDrill.srs.interval}d · easiness {activeDrill.srs.easiness.toFixed(2)} · reps{" "}
                    {activeDrill.srs.repetitions}
                  </p>
                )}
              </div>
            ) : (
              <div className="mt-3 space-y-2">
                <p className="text-xs text-gray-300 font-mono min-h-5" aria-live="polite">
                  {boardMoves.length > 0 ? boardMoves.join(" ") : "Starting position"}
                </p>
                <div className="flex gap-2">
                  <input
                    value={lineName}
                    onChange={(e) => setLineName(e.target.value)}
                    placeholder="Line name (optional)"
                    aria-label="Line name"
                    className="flex-1 px-3 py-2 text-sm rounded-xl bg-gray-900/70 border border-gray-700/50 text-white placeholder:text-gray-500"
                  />
                  <button
                    onClick={handleSaveLine}
                    className="px-4 py-2 text-sm font-bold rounded-xl bg-gradient-to-r from-emerald-500 to-teal-600 text-white hover:from-emerald-600 hover:to-teal-700"
                  >
                    Save to {color}
                  </button>
                </div>
                <button
                  onClick={resetBoard}
                  className="w-full px-3 py-2 text-sm rounded-xl bg-gray-700/60 text-gray-200 hover:bg-gray-600/60"
                >
                  Reset board
                </button>
              </div>
            )}
          </div>

          <div className="bg-gray-800/60 rounded-xl border border-gray-700/50 p-4">
            <div className="flex items-center justify-between mb-2">
              <h2 className="text-lg font-bold text-white capitalize">{color} repertoire ({lines.length})</h2>
              {selectedOpening && (
                <span className="text-[11px] font-mono px-2 py-0.5 rounded bg-teal-500/15 text-teal-300 border border-teal-500/30">
                  {selectedOpening.eco}
                </span>
              )}
            </div>
            {selectedOpening && (
              <p className="text-xs text-gray-400 mb-3">
                {selectedOpening.name} · {selectedOpening.blurb}
              </p>
            )}
            <div className="flex gap-2 mb-3">
              <button
                onClick={handleExport}
                className="flex-1 px-3 py-2 text-sm rounded-xl bg-gray-700/60 text-white hover:bg-gray-600/60"
              >
                Export PGN
              </button>
              <button
                onClick={() => fileRef.current?.click()}
                className="flex-1 px-3 py-2 text-sm rounded-xl bg-gray-700/60 text-white hover:bg-gray-600/60"
              >
                Import PGN
              </button>
              <input
                ref={fileRef}
                type="file"
                accept=".pgn,.txt"
                className="hidden"
                aria-label="Import PGN file"
                onChange={(e) => {
                  const f = e.target.files?.[0];
                  if (f) void handleImportFile(f);
                  e.target.value = "";
                }}
              />
            </div>
            {lines.length === 0 ? (
              <p className="text-sm text-gray-500">
                No {color} lines yet. Play moves on the board and save them, or import a PGN file.
              </p>
            ) : (
              <ul className="space-y-2 max-h-[480px] overflow-y-auto pr-1">
                {lines.map((l) => {
                  const due = l.srs.nextReview <= Date.now();
                  return (
                    <li
                      key={l.id}
                      className="p-3 rounded-xl border border-gray-700/50 bg-gray-900/60"
                    >
                      <div className="flex items-center justify-between gap-2">
                        <p className="text-sm font-semibold text-white truncate">{l.name}</p>
                        <span
                          className={`text-[11px] px-2 py-0.5 rounded-full border ${
                            due
                              ? "text-orange-300 bg-orange-500/10 border-orange-500/30"
                              : "text-emerald-300 bg-emerald-500/10 border-emerald-500/30"
                          }`}
                        >
                          {due ? "due" : `${l.srs.interval}d`}
                        </span>
                      </div>
                      <p className="text-[11px] font-mono text-teal-300 mt-1">{l.eco}</p>
                      <p className="text-xs text-gray-400 font-mono truncate mt-1">{l.moves.join(" ")}</p>
                      <div className="flex gap-2 mt-2">
                        <button
                          onClick={() => {
                            const idx = drillQueue.findIndex((d) => d.id === l.id);
                            setDrillIndex(idx >= 0 ? idx : 0);
                            setMode("drill");
                          }}
                          className="flex-1 px-2 py-1.5 text-xs rounded-lg bg-emerald-500/15 border border-emerald-500/30 text-emerald-300 hover:bg-emerald-500/25"
                        >
                          Drill
                        </button>
                        <button
                          onClick={() => {
                            setBoardMoves(l.moves);
                            const c = new Chess();
                            for (const san of l.moves) {
                              try {
                                c.move(san);
                              } catch {
                                break;
                              }
                            }
                            gameRef.current = c;
                            setBoardFen(c.fen());
                            setMode("build");
                          }}
                          className="flex-1 px-2 py-1.5 text-xs rounded-lg bg-gray-700/60 text-gray-200 hover:bg-gray-600/60"
                        >
                          Load
                        </button>
                        <button
                          onClick={() => handleDeleteLine(l.id)}
                          aria-label={`Delete ${l.name}`}
                          className="flex-1 px-2 py-1.5 text-xs rounded-lg bg-red-500/10 border border-red-500/30 text-red-300 hover:bg-red-500/20"
                        >
                          Delete
                        </button>
                      </div>
                      {l.srs.lastReviewed && (
                        <p className="text-[11px] text-gray-500 mt-1">
                          EF {l.srs.easiness.toFixed(2)} · reps {l.srs.repetitions} · lapses {l.srs.lapses}
                        </p>
                      )}
                    </li>
                  );
                })}
              </ul>
            )}
            <p className="text-[11px] text-gray-500 mt-2">
              Stored locally, synced to your profile when signed in.
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}
