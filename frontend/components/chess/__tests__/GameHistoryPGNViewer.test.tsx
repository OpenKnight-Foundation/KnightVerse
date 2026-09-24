/**
 * Regression tests for issue #1228.
 *
 * `GameHistoryPGNViewer` used to default `pgn` to the bundled `MOCK_PGN`, so
 * every render — including production ones — silently replayed a fabricated
 * game. The component now requires the real PGN from the caller, and the sample
 * is only an exported fixture.
 */

import React from "react";
import type { ComponentProps } from "react";
import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";

// The board widget pulls in third-party rendering that is irrelevant here.
vi.mock("../ChessboardComponent", () => ({
  default: () => React.createElement("div", { "data-testid": "chessboard" }),
}));

import { GameHistoryPGNViewer, MOCK_PGN } from "../GameHistoryPGNViewer";

/** Fool's mate — the shortest legal game, with distinctive headers. */
const REAL_GAME_PGN = `[Event "Testnet Rated Game"]
[Site "knightverse.app"]
[Date "2026.04.01"]
[White "AminaReplays"]
[Black "BilalReplays"]
[Result "0-1"]

1. f3 e5 2. g4 Qh4# 0-1`;

describe("GameHistoryPGNViewer (#1228)", () => {
  beforeEach(() => {
    // jsdom does not implement scrollIntoView, which the component calls when
    // the active move changes.
    Element.prototype.scrollIntoView = vi.fn();
  });

  it("replays the PGN supplied by the caller instead of the sample fixture", () => {
    render(<GameHistoryPGNViewer pgn={REAL_GAME_PGN} />);

    expect(screen.getByText(/AminaReplays/)).toBeInTheDocument();
    expect(screen.getByText(/BilalReplays/)).toBeInTheDocument();
    expect(screen.getByText("0-1")).toBeInTheDocument();
    expect(screen.getByText("2026-04-01")).toBeInTheDocument();

    // Nothing from the bundled sample leaks into a real game's replay.
    expect(screen.queryByText(/Morphy/)).not.toBeInTheDocument();
    expect(screen.queryByText(/Duke of Brunswick/)).not.toBeInTheDocument();
    expect(screen.queryByText("1858-10-21")).not.toBeInTheDocument();
  });

  it("replays the caller's moves — move list, count and board", () => {
    render(<GameHistoryPGNViewer pgn={REAL_GAME_PGN} />);

    expect(screen.getByText(/^0 \/ 4 moves$/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "f3" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Qh4#" })).toBeInTheDocument();
    expect(screen.getByTestId("chessboard")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Next move" })).toBeEnabled();
  });

  it("renders the sample fixture when it is passed explicitly", () => {
    render(<GameHistoryPGNViewer pgn={MOCK_PGN} />);

    // The fixture must be replayable — it used to contain an illegal move.
    expect(screen.queryByText(/Invalid PGN/)).not.toBeInTheDocument();
    expect(screen.getByText(/Morphy, Paul/)).toBeInTheDocument();
    expect(screen.getByText(/Duke of Brunswick/)).toBeInTheDocument();
    expect(screen.getByText(/^0 \/ 33 moves$/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Go to end" })).toBeEnabled();
  });

  it("keeps `pgn` required so the component can never silently default", () => {
    // Compile-time guard, enforced by `tsc` during `next build`: if `pgn` ever
    // becomes optional (or regains a default), this stops compiling.
    type PgnProp = ComponentProps<typeof GameHistoryPGNViewer>["pgn"];
    const pgnIsRequired: undefined extends PgnProp ? never : true = true;

    expect(pgnIsRequired).toBe(true);
  });
});
