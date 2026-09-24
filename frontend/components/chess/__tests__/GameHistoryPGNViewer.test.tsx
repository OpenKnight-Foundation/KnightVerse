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
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";

// The board widget pulls in third-party rendering that is irrelevant here.
vi.mock("../ChessboardComponent", () => ({
  default: () => React.createElement("div", { "data-testid": "chessboard" }),
}));

vi.mock("next/image", () => ({
  default: ({ src, alt, ...rest }: { src?: string; alt?: string; [key: string]: unknown }) =>
    React.createElement("img", {
      src: typeof src === "string" ? src : "",
      alt: typeof alt === "string" ? alt : "",
      ...rest,
    }),
}));

// SVG chess piece assets — plain string module exports, no compiler needed
vi.mock("@/components/chess/chesspieces/white-king.svg", () => ({ default: "" }));
vi.mock("@/components/chess/chesspieces/white-queen.svg", () => ({ default: "" }));
vi.mock("@/components/chess/chesspieces/white-bishop.svg", () => ({ default: "" }));
vi.mock("@/components/chess/chesspieces/white-knight.svg", () => ({ default: "" }));
vi.mock("@/components/chess/chesspieces/white-rook.svg", () => ({ default: "" }));
vi.mock("@/components/chess/chesspieces/white-pawn.svg", () => ({ default: "" }));
vi.mock("@/components/chess/chesspieces/black-king.svg", () => ({ default: "" }));
vi.mock("@/components/chess/chesspieces/black-queen.svg", () => ({ default: "" }));
vi.mock("@/components/chess/chesspieces/black-bishop.svg", () => ({ default: "" }));
vi.mock("@/components/chess/chesspieces/black-knight.svg", () => ({ default: "" }));
vi.mock("@/components/chess/chesspieces/black-rook.svg", () => ({ default: "" }));
vi.mock("@/components/chess/chesspieces/black-pawn.svg", () => ({ default: "" }));

vi.mock("@/context/ThemeContext", () => ({
  useBoardTheme: () => ({
    colors: { light: "#f0d9b5", dark: "#b58863", selected: "#a5c0ff", lastMove: "#90d26d" },
  }),
}));

vi.mock("@/context/GamePreferencesContext", () => ({
  useGamePreferences: () => ({
    preferences: {
      pieceInputMethod: "both",
      autoQueen: "always",
      showLegalMoveDots: "enabled",
      confirmMoveCorrespondence: false,
      boardCoordinates: "inside",
      pieceSet: "neo",
    },
    setPreference: () => {},
    resetPreferences: () => {},
  }),
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

/**
 * Fidelity is asserted against the sample above; the export test below needs a
 * fixture that carries Elo headers, so it keeps its own (a test-local copy of the
 * sample this component used to bundle).
 */
const PDF_EXPORT_PGN = `[Event "KnightVerse Rated Game"]
[Site "knightverse.app"]
[Date "2026.03.26"]
[White "GABC...XYZ"]
[Black "GDEF...UVW"]
[Result "1-0"]
[WhiteElo "1280"]
[BlackElo "1263"]
[TimeControl "300+3"]

1. e4 { [%clk 0:05:00] } e5 { [%clk 0:05:00] }
2. Nf3 { [%clk 0:04:58] } d6 { [%clk 0:04:57] }
3. d4 { [%clk 0:04:55] } Bg4 { [%clk 0:04:54] }
4. dxe5 { [%clk 0:04:52] } Bxf3 { [%clk 0:04:51] }
5. Qxf3 { [%clk 0:04:50] } dxe5 { [%clk 0:04:49] }
6. Bc4 { [%clk 0:04:47] } Nf6 { [%clk 0:04:46] }
7. Qb3 { [%clk 0:04:45] } Qe7 { [%clk 0:04:43] }
8. Nc3 { [%clk 0:04:43] } c6 { [%clk 0:04:41] }
9. Bg5 { [%clk 0:04:41] } b5 { [%clk 0:04:39] }
10. Nxb5 { [%clk 0:04:38] } cxb5 { [%clk 0:04:37] }
11. Bxb5+ { [%clk 0:04:36] } Nbd7 { [%clk 0:04:35] }
12. O-O-O { [%clk 0:04:34] } Rd8 { [%clk 0:04:33] }
13. Rxd7 { [%clk 0:04:32] } Rxd7 { [%clk 0:04:30] }
14. Rd1 { [%clk 0:04:30] } Qe6 { [%clk 0:04:28] }
15. Bxd7+ { [%clk 0:04:28] } Nxd7 { [%clk 0:04:26] }
16. Qb8+ { [%clk 0:04:26] } Nxb8 { [%clk 0:04:24] }
17. Rd8# { [%clk 0:04:24] } 1-0`;

describe("GameHistoryPGNViewer — PDF scoresheet export (FE-79)", () => {
  let anchorClick: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    // jsdom does not implement these — define them so the viewer can call them
    Object.defineProperty(Element.prototype, "scrollIntoView", {
      configurable: true,
      value: vi.fn(),
    });
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

  it("renders an Export as PDF action on the replay header", async () => {
    render(<GameHistoryPGNViewer pgn={PDF_EXPORT_PGN} />);
    expect(
      screen.getByRole("heading", { name: /game replay/i }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /pdf scoresheet/i }),
    ).toBeInTheDocument();
  });

  it("downloads a valid scoresheet blob matching the rendered game", async () => {
    const { create, revoke, captured } = stubObjectUrls();

    render(<GameHistoryPGNViewer pgn={PDF_EXPORT_PGN} />);
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
    expect(text).toContain("GABC...XYZ \\(1280\\)");
    expect(text).toContain("GDEF...UVW \\(1263\\)");
    expect(text).toContain("2026.03.26");
    expect(text).toContain("1-0");
    // Moves from the PGN body
    expect(text).toContain("Qb8+");
    expect(text).toContain("Rd8#");
    expect(text).toContain("Final Position");
  });
});
