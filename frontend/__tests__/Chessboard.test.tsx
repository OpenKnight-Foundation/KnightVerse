import React from "react";
import { render, screen, fireEvent } from "@testing-library/react";
import Chessboard from "../components/chess/Chessboard";

describe("Chessboard", () => {
  it("renders the chessboard", () => {
    const onMove = jest.fn();
    render(<Chessboard position="start" onMove={onMove} />);
    const chessboard = screen.getByRole("grid");
    expect(chessboard).toBeInTheDocument();
  });

  it("highlights legal moves for a selected piece", () => {
    const onMove = jest.fn();
    render(<Chessboard position="start" onMove={onMove} />);

    // Click on the e2 pawn
    fireEvent.click(screen.getByLabelText("e2 White Pawn"));

    // The legal moves for the e2 pawn are e3 and e4
    const e3Square = screen.getByLabelText("e3 empty");
    const e4Square = screen.getByLabelText("e4 empty");

    expect(e3Square).toHaveClass("bg-green-500/50");
    expect(e4Square).toHaveClass("bg-green-500/50");
  });

  it("rejects an illegal move", () => {
    const onMove = jest.fn();
    render(<Chessboard position="start" onMove={onMove} />);

    // Click on the e2 pawn
    fireEvent.click(screen.getByLabelText("e2 White Pawn"));

    // Attempt to move the e2 pawn to e5, which is an illegal move
    fireEvent.click(screen.getByLabelText("e5 empty"));

    // The onMove callback should not have been called
    expect(onMove).not.toHaveBeenCalled();
  });
});
