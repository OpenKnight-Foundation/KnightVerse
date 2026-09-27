/**
 * Repertoire store: White/Black trees in localStorage + profile sync stub (#1112 FE-53).
 * PGN import/export via chess.js.
 */
"use client";

import { Chess } from "chess.js";
import {
  initialCardState,
  type SrsCardState,
} from "./spacedRepetition";
import { API_BASE } from "./api";

export type RepertoireColor = "white" | "black";

export interface RepertoireLine {
  id: string;
  name: string;
  eco: string;
  /** SAN moves from the start position. */
  moves: string[];
  notes?: string;
  srs: SrsCardState;
  updatedAt: number;
}

export interface RepertoireState {
  white: RepertoireLine[];
  black: RepertoireLine[];
}

export const REPERTOIRE_STORAGE_KEY = "knightverse_repertoire_v1";

export const EMPTY_REPERTOIRE: RepertoireState = { white: [], black: [] };

function uid(): string {
  return `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}

export function createLine(input: {
  name: string;
  eco: string;
  moves: string[];
  notes?: string;
}): RepertoireLine {
  return {
    id: uid(),
    name: input.name,
    eco: input.eco,
    moves: [...input.moves],
    notes: input.notes ?? "",
    srs: initialCardState(),
    updatedAt: Date.now(),
  };
}

export function loadRepertoire(): RepertoireState {
  try {
    const raw = localStorage.getItem(REPERTOIRE_STORAGE_KEY);
    if (!raw) return EMPTY_REPERTOIRE;
    const parsed = JSON.parse(raw) as Partial<RepertoireState>;
    return {
      white: Array.isArray(parsed.white) ? parsed.white : [],
      black: Array.isArray(parsed.black) ? parsed.black : [],
    };
  } catch {
    return EMPTY_REPERTOIRE;
  }
}

export function saveRepertoire(state: RepertoireState): void {
  try {
    localStorage.setItem(REPERTOIRE_STORAGE_KEY, JSON.stringify(state));
  } catch {
    // storage unavailable — drill still works in-memory
  }
}

/**
 * Best-effort sync to user profile. Backend `/v1/repertoires` does not exist
 * yet, so failures are swallowed and localStorage stays source of truth.
 */
export async function syncRepertoireToProfile(
  state: RepertoireState,
): Promise<boolean> {
  try {
    const token = localStorage.getItem("access_token");
    if (!token) return false;
    const res = await fetch(`${API_BASE}/v1/repertoires/sync`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${token}`,
      },
      body: JSON.stringify(state),
    });
    return res.ok;
  } catch {
    return false;
  }
}

/** Parse a PGN string into SAN lines (one entry per game). */
export function importPgnToLines(pgn: string): string[][] {
  const games = pgn
    .split(/\r?\n\r?\n(?=\[Event )/)
    .map((g) => g.trim())
    .filter(Boolean);
  const lines: string[][] = [];
  for (const game of games) {
    try {
      const chess = new Chess();
      chess.loadPgn(game);
      const history = chess.history();
      if (history.length > 0) lines.push(history);
    } catch {
      // skip malformed games, keep the valid ones
    }
  }
  return lines;
}

/** Export repertoire lines to a multi-game PGN string. */
export function exportLinesToPgn(
  lines: RepertoireLine[],
  color: RepertoireColor,
): string {
  return lines
    .map((line) => {
      const chess = new Chess();
      for (const san of line.moves) {
        try {
          chess.move(san);
        } catch {
          break;
        }
      }
      const header =
        `[Event "KnightVerse Repertoire"]\n` +
        `[Site "KnightVerse"]\n` +
        `[Date "${new Date().toISOString().slice(0, 10).replace(/-/g, ".")}"]\n` +
        `[White "${color === "white" ? "Repertoire" : "Opponent"}"]\n` +
        `[Black "${color === "black" ? "Repertoire" : "Opponent"}"]\n` +
        `[ECO "${line.eco}"]\n` +
        `[Opening "${line.name}"]\n` +
        `[Result "*"]`;
      const moves = line.moves
        .map((san, idx) =>
          idx % 2 === 0 ? `${idx / 2 + 1}. ${san}` : san,
        )
        .join(" ");
      return `${header}\n\n${moves} *`;
    })
    .join("\n\n\n");
}
