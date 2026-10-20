/**
 * Tests for ChessboardWebGL3D — FE-54 (#1113)
 *
 * Covers:
 * - Renders the data-testid container
 * - Material picker renders all 4 materials
 * - 2D fallback is shown when WebGL is unavailable
 * - Toggle 3D/2D button exists
 * - FEN pieces are shown in 2D fallback
 * - onMove callback is called from the 2D fallback board
 * - Material palette constants are exported correctly
 */

import React from "react";
import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  ChessboardWebGL3D,
  MATERIAL_PALETTES,
  type BoardMaterial,
} from "../ChessboardWebGL3D";

// next/dynamic is used for the 3D canvas; in jsdom there is no WebGL,
// so the component always falls back to the 2D board.
vi.mock("next/dynamic", () => ({
  default: () => () => null,
}));

const STARTING_FEN = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

describe("ChessboardWebGL3D (FE-54 / #1113)", () => {
  beforeEach(() => {
    // jsdom does not support WebGL — simulate absence of getContext
    const original = HTMLCanvasElement.prototype.getContext;
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
    return () => {
      HTMLCanvasElement.prototype.getContext = original;
    };
  });

  it("renders the chessboard container", () => {
    render(<ChessboardWebGL3D fen={STARTING_FEN} />);
    expect(screen.getByTestId("chessboard-webgl-3d")).toBeInTheDocument();
  });

  it("renders all 4 material buttons", () => {
    render(<ChessboardWebGL3D fen={STARTING_FEN} />);
    const materials: BoardMaterial[] = ["Marble", "Obsidian", "HolographicNeon", "ClassicMahogany"];
    for (const m of materials) {
      expect(
        screen.getByRole("button", { name: new RegExp(MATERIAL_PALETTES[m].label, "i") })
      ).toBeInTheDocument();
    }
  });

  it("shows the 2D fallback warning when WebGL is unavailable", () => {
    render(<ChessboardWebGL3D fen={STARTING_FEN} />);
    expect(screen.getByText(/webgl unavailable/i)).toBeInTheDocument();
  });

  it("shows pieces in the 2D fallback board", () => {
    render(<ChessboardWebGL3D fen={STARTING_FEN} interactive />);
    // White King ♔ at e1
    expect(screen.getByLabelText(/e1, ♔/i)).toBeInTheDocument();
    // Black Queen ♛ at d8
    expect(screen.getByLabelText(/d8, ♛/i)).toBeInTheDocument();
  });

  it("calls onMove when a piece is clicked and a target is selected", () => {
    const onMove = vi.fn().mockReturnValue(true);
    render(<ChessboardWebGL3D fen={STARTING_FEN} onMove={onMove} interactive />);

    // Click e2 (white pawn)
    const e2 = screen.getByLabelText(/e2, ♙/i);
    fireEvent.click(e2);

    // Click e4 (empty square target)
    const e4 = screen.getByLabelText(/^e4$/i);
    fireEvent.click(e4);

    expect(onMove).toHaveBeenCalledWith("e2", "e4");
  });

  it("toggles to Try 3D button when in 2D mode", () => {
    render(<ChessboardWebGL3D fen={STARTING_FEN} />);
    const toggleBtn = screen.getByRole("button", { name: /try 3d/i });
    expect(toggleBtn).toBeInTheDocument();
  });

  it("exports MATERIAL_PALETTES with all 4 entries", () => {
    expect(Object.keys(MATERIAL_PALETTES)).toHaveLength(4);
    expect(MATERIAL_PALETTES.Marble.light).toBeTruthy();
    expect(MATERIAL_PALETTES.Obsidian.dark).toBeTruthy();
    expect(MATERIAL_PALETTES.HolographicNeon.accent).toBeTruthy();
    expect(MATERIAL_PALETTES.ClassicMahogany.label).toBe("Classic Mahogany");
  });

  it("does not call onMove when not interactive", () => {
    const onMove = vi.fn();
    render(
      <ChessboardWebGL3D fen={STARTING_FEN} onMove={onMove} interactive={false} />
    );
    // In non-interactive mode clicking squares does nothing
    const e2 = screen.getByLabelText(/e2, ♙/i);
    fireEvent.click(e2);
    expect(onMove).not.toHaveBeenCalled();
  });
});
