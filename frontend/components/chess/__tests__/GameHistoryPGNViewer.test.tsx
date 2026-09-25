/**
 * Component tests for GameHistoryPGNViewer.
 *
 * - Regression tests for issue #1228: `GameHistoryPGNViewer` used to default
 *   `pgn` to the bundled `MOCK_PGN`, so every render — including production
 *   ones — silently replayed a fabricated game. The component now requires the
 *   real PGN from the caller, and the sample is only an exported fixture.
 * - FE-79: the "Export as PDF" action triggers a browser download of a valid
 *   PDF blob whose content matches the rendered game (metadata + moves).
 */

import React from "react";
import type { ComponentProps } from "react";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
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

beforeEach(() => {
  // jsdom does not implement scrollIntoView, which the component calls when
  // the active move changes.
  Object.defineProperty(Element.prototype, "scrollIntoView", {
    configurable: true,
    value: vi.fn(),
  });
});

describe("GameHistoryPGNViewer (#1228)", () => {
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

describe("GameHistoryPGNViewer — PDF scoresheet export (FE-79)", () => {
  let anchorClick: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    anchorClick = vi.fn();
    Object.defineProperty(HTMLAnchorElement.prototype, "click", {
      configurable: true,
      writable: true,
      value: anchorClick,
    });
  });

  function stubObjectUrls(): {
    create: ReturnType<typeof vi.fn>;
    revoke: ReturnType<typeof vi.fn>;
    captured: () => Blob | null;
  } {
    let captured: Blob | null = null;
    const create = vi.fn((blob: Blob) => {
      captured = blob;
      return "blob:mock";
    });
    const revoke = vi.fn();
    Object.defineProperty(URL, "createObjectURL", { configurable: true, value: create });
    Object.defineProperty(URL, "revokeObjectURL", { configurable: true, value: revoke });
    return { create, revoke, captured: () => captured };
  }

  it("renders an Export as PDF action on the replay header", () => {
    render(<GameHistoryPGNViewer pgn={MOCK_PGN} />);
    expect(
      screen.getByRole("heading", { name: /game replay/i }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /pdf scoresheet/i }),
    ).toBeInTheDocument();
  });

  it("downloads a valid scoresheet blob matching the rendered game", async () => {
    const { create, revoke, captured } = stubObjectUrls();

    render(<GameHistoryPGNViewer pgn={MOCK_PGN} />);
    // Parsing runs in an effect — wait for the move list to appear
    await waitFor(() =>
      expect(screen.getByText(/Rd8#/)).toBeInTheDocument(),
    );

    const exportButton = screen.getByRole("button", { name: /pdf scoresheet/i });
    fireEvent.click(exportButton);

    expect(create).toHaveBeenCalledTimes(1);
    expect(anchorClick).toHaveBeenCalledTimes(1);
    expect(revoke).toHaveBeenCalledTimes(1);

    const blob = captured();
    expect(blob).not.toBeNull();
    expect(blob!.type).toBe("application/pdf");

    const text = await blob!.text();
    expect(text.startsWith("%PDF-1.4")).toBe(true);
    expect(text.endsWith("%%EOF\n")).toBe(true);
    // Metadata from the PGN headers
    expect(text).toContain("Morphy, Paul");
    expect(text).toContain("Duke of Brunswick");
    expect(text).toContain("1858.10.21");
    expect(text).toContain("1-0");
    // Moves from the PGN body
    expect(text).toContain("Qb8+");
    expect(text).toContain("Rd8#");
    expect(text).toContain("Final Position");
  });
});
