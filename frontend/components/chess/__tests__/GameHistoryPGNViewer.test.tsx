/**
 * Component tests for GameHistoryPGNViewer (FE-79) — PDF scoresheet export.
 *
 * Verifies the "Export as PDF" action triggers a browser download of a valid
 * PDF blob whose content matches the rendered game (metadata + moves).
 */

import React from "react";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";

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

import { GameHistoryPGNViewer, MOCK_PGN } from "@/components/chess/GameHistoryPGNViewer";

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
    render(<GameHistoryPGNViewer />);
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