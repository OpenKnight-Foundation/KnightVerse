/**
 * Static ECO / opening dataset + classification helpers (#1112 FE-53).
 * Offline-first so the explorer works without network.
 */

export interface OpeningInfo {
  eco: string;
  name: string;
  /** Canonical move sequence in SAN. */
  moves: string[];
  /** Short description shown in the explorer. */
  blurb: string;
}

export interface OpeningStats {
  white: number;
  draws: number;
  black: number;
  games: number;
}

/** Curated starter book — covers the most common club lines. */
export const ECO_BOOK: OpeningInfo[] = [
  { eco: "C50", name: "Italian Game: Giuoco Piano", moves: ["e4", "e5", "Nf3", "Nc6", "Bc4", "Bc5"], blurb: "Classical development with quick castling." },
  { eco: "C60", name: "Ruy Lopez: Cozio Defense", moves: ["e4", "e5", "Nf3", "Nc6", "Bb5", "Nge7"], blurb: "Solid Spanish setup delaying ...a6." },
  { eco: "C61", name: "Ruy Lopez: Bird Defense", moves: ["e4", "e5", "Nf3", "Nc6", "Bb5", "Nd4"], blurb: "Early central counter with ...Nd4." },
  { eco: "B20", name: "Sicilian Defense", moves: ["e4", "c5"], blurb: "Fighting asymmetric reply to 1.e4." },
  { eco: "B90", name: "Sicilian: Najdorf", moves: ["e4", "c5", "Nf3", "d6", "d4", "cxd4", "Nxd4", "Nf6", "Nc3", "a6"], blurb: "The sharpest Sicilian, Kasparov's weapon." },
  { eco: "B01", name: "Scandinavian Defense", moves: ["e4", "d5"], blurb: "Immediate central strike." },
  { eco: "C00", name: "French Defense", moves: ["e4", "e6"], blurb: "Solid counter-attacking structure." },
  { eco: "C01", name: "French: Exchange Variation", moves: ["e4", "e6", "d4", "d5", "exd5", "exd5", "Nf3"], blurb: "Symmetrical, good for learning plans." },
  { eco: "C42", name: "Petrov Defense", moves: ["e4", "e5", "Nf3", "Nf6"], blurb: "Symmetrical solid reply." },
  { eco: "B07", name: "Pirc Defense", moves: ["e4", "d6", "d4", "Nf6", "Nc3", "g6"], blurb: "Hypermodern fianchetto system." },
  { eco: "D06", name: "Queen's Gambit Declined", moves: ["d4", "d5", "c4", "e6"], blurb: "Classical central solidity." },
  { eco: "D02", name: "London System", moves: ["d4", "d5", "Bf4", "Nf6", "e3", "e6", "Nf3"], blurb: "System opening, same setup vs almost anything." },
  { eco: "E60", name: "King's Indian Defense", moves: ["d4", "Nf6", "c4", "g6"], blurb: "Counter-attacking kingside fianchetto." },
  { eco: "A10", name: "English Opening", moves: ["c4"], blurb: "Flank control with flexible transpositions." },
  { eco: "B10", name: "Caro-Kann Defense", moves: ["e4", "c6"], blurb: "Solid, weakness-free pawn structure." },
  { eco: "C33", name: "King's Gambit Accepted", moves: ["e4", "e5", "f4", "exf4"], blurb: "Romantic attacking gambit." },
];

/** Representative master-level stats used offline + as fallback. */
export const MASTER_STATS_FALLBACK: Record<string, OpeningStats> = {
  C50: { white: 38, draws: 34, black: 28, games: 124810 },
  C60: { white: 39, draws: 35, black: 26, games: 98244 },
  C61: { white: 36, draws: 33, black: 31, games: 18712 },
  B20: { white: 40, draws: 29, black: 31, games: 210553 },
  B90: { white: 38, draws: 31, black: 31, games: 88410 },
  B01: { white: 42, draws: 30, black: 28, games: 31208 },
  C00: { white: 37, draws: 32, black: 31, games: 140977 },
  C01: { white: 35, draws: 40, black: 25, games: 22140 },
  C42: { white: 34, draws: 41, black: 25, games: 45210 },
  B07: { white: 41, draws: 29, black: 30, games: 38902 },
  D06: { white: 36, draws: 38, black: 26, games: 110544 },
  D02: { white: 38, draws: 34, black: 28, games: 76530 },
  E60: { white: 40, draws: 30, black: 30, games: 95412 },
  A10: { white: 37, draws: 34, black: 29, games: 68900 },
  B10: { white: 39, draws: 31, black: 30, games: 77420 },
  C33: { white: 44, draws: 24, black: 32, games: 12980 },
};

function normalizeSans(moves: string[]): string[] {
  return moves.map((m) => m.replace(/[+#!?]+$/g, "").trim());
}

/**
 * Classify a SAN line to the longest matching ECO entry (prefix match).
 * Returns null when nothing matches.
 */
export function classifyOpening(moves: string[]): OpeningInfo | null {
  const line = normalizeSans(moves);
  if (line.length === 0) return null;
  let best: OpeningInfo | null = null;
  for (const entry of ECO_BOOK) {
    const eco = normalizeSans(entry.moves);
    if (eco.length > line.length) continue;
    const matches = eco.every((m, i) => line[i] === m);
    if (matches && (!best || eco.length > best.moves.length)) best = entry;
  }
  return best;
}

export function statsForEco(eco: string): OpeningStats {
  return (
    MASTER_STATS_FALLBACK[eco] ?? { white: 34, draws: 33, black: 33, games: 5000 }
  );
}

export function winRateBar(stats: OpeningStats): {
  whitePct: number;
  drawPct: number;
  blackPct: number;
} {
  const total = stats.white + stats.draws + stats.black || 1;
  return {
    whitePct: Math.round((stats.white / total) * 100),
    drawPct: Math.round((stats.draws / total) * 100),
    blackPct: Math.round((stats.black / total) * 100),
  };
}
