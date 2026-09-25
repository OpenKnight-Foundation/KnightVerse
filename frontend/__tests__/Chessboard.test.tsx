import React from "react";
import { render, screen, fireEvent, within } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import Chessboard from "../components/chess/Chessboard";
import { GamePreferencesProvider } from "@/context/GamePreferencesContext";
import { BoardThemeProvider } from "@/context/ThemeContext";

const Providers = ({ children }: { children: React.ReactNode }) => (
  <BoardThemeProvider>
    <GamePreferencesProvider>{children}</GamePreferencesProvider>
  </BoardThemeProvider>
);

const renderBoard = (onMove = vi.fn()) =>
  render(<Chessboard position="start" onMove={onMove} />, { wrapper: Providers });

describe("Chessboard", () => {
  it("renders the chessboard", () => {
    renderBoard();
    expect(screen.getByRole("grid")).toBeInTheDocument();
    expect(screen.getAllByRole("gridcell")).toHaveLength(64);
  });

  it("highlights legal moves for a selected piece", () => {
    renderBoard();

    // Click on the e2 pawn
    fireEvent.click(screen.getByLabelText(/^e2, White Pawn/));

    // The legal moves for the e2 pawn are e3 and e4
    const e3Square = screen.getByLabelText(/^e3, empty/);
    const e4Square = screen.getByLabelText(/^e4, empty/);
    expect(within(e3Square).getByTestId("legal-move-dot")).toBeInTheDocument();
    expect(within(e4Square).getByTestId("legal-move-dot")).toBeInTheDocument();
    expect(screen.getAllByTestId("legal-move-dot")).toHaveLength(2);
  });

  it("rejects an illegal move", () => {
    const onMove = vi.fn();
    renderBoard(onMove);

    // Click on the e2 pawn
    fireEvent.click(screen.getByLabelText(/^e2, White Pawn/));

    // Attempt to move the e2 pawn to e5, which is an illegal move
    fireEvent.click(screen.getByLabelText(/^e5, empty/));

    // The onMove callback should not have been called
    expect(onMove).not.toHaveBeenCalled();
  });
});
