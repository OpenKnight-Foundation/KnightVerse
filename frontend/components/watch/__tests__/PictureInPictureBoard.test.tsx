import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";

import type { PipRect, PipWindowState, ViewportSize } from "@/lib/pipWindow";

import { PictureInPictureBoard } from "../PictureInPictureBoard";
import { PipWindowLayer } from "../PipWindowLayer";

vi.mock("@/hook/useSpectatorSocket", () => ({
  useSpectatorSocket: () => ({
    status: "connected",
    gameState: {
      fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
      moves: [{ from: "e2", to: "e4", san: "e4", color: "w" }],
      whiteTime: 300,
      blackTime: 300,
      status: "playing",
      spectatorCount: 5,
      white: { address: "GWHITE1234567890", elo: 1500 },
      black: { address: "GBLACK1234567890", elo: 1450 },
    },
    disconnect: vi.fn(),
    reconnect: vi.fn(),
  }),
}));

const VIEWPORT: ViewportSize = { width: 1280, height: 800 };
const RECT: PipRect = { x: 100, y: 100, width: 240, height: 240 };
const START_FEN = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

function renderBoard(overrides: Record<string, unknown> = {}) {
  const handlers = {
    onFocus: vi.fn(),
    onClose: vi.fn(),
    onExpand: vi.fn(),
    onRectChange: vi.fn(),
  };

  render(
    <PictureInPictureBoard
      gameId="game-1"
      rect={RECT}
      zIndex={3}
      viewport={VIEWPORT}
      white={{ address: "GWHITE1234567890", elo: 1500 }}
      black={{ address: "GBLACK1234567890", elo: 1450 }}
      fen={START_FEN}
      moves={[{ from: "e2", to: "e4", san: "e4", color: "w" }]}
      status="connected"
      {...handlers}
      {...overrides}
    />,
  );

  return handlers;
}

describe("PictureInPictureBoard", () => {
  it("renders an accessible floating dialog for the pinned game", () => {
    renderBoard();
    const dialog = screen.getByTestId("pip-window-game-1");
    expect(dialog).toHaveAttribute("role", "dialog");
    expect(dialog.getAttribute("aria-label")).toContain("game-1");
    expect(dialog.style.width).toBe("240px");
  });

  it("renders an 8x8 low-overhead board with no canvas", () => {
    renderBoard();
    const board = screen.getByTestId("pip-board-game-1");
    expect(board.children).toHaveLength(64);
    expect(board.querySelector("canvas")).toBeNull();
  });

  it("shows the latest move and the material evaluation", () => {
    renderBoard();
    expect(screen.getByTestId("pip-move-game-1").textContent).toContain("1.e4");
    expect(screen.getByTestId("pip-eval-game-1").textContent).toBe("0.0");
    expect(screen.getByTestId("pip-eval-bar-game-1")).toHaveAttribute("aria-valuenow", "50");
  });

  it("reflects a supplied centipawn evaluation in the mini bar", () => {
    renderBoard({ evaluationCp: 300 });
    expect(screen.getByTestId("pip-eval-game-1").textContent).toBe("+3.0");
    expect(Number(screen.getByTestId("pip-eval-bar-game-1").getAttribute("aria-valuenow"))).toBeGreaterThan(50);
  });

  it("closes and expands via the title bar controls", () => {
    const handlers = renderBoard();
    fireEvent.click(screen.getByLabelText("Close picture-in-picture for game game-1"));
    expect(handlers.onClose).toHaveBeenCalledWith("game-1");

    fireEvent.click(screen.getByLabelText("Expand game game-1"));
    expect(handlers.onExpand).toHaveBeenCalledWith("game-1");
  });

  it("nudges the window with the arrow keys and closes on Escape", () => {
    const handlers = renderBoard();
    const dialog = screen.getByTestId("pip-window-game-1");

    fireEvent.keyDown(dialog, { key: "ArrowRight" });
    expect(handlers.onRectChange).toHaveBeenCalledTimes(1);
    expect(handlers.onRectChange.mock.calls[0][0]).toBe("game-1");
    expect(handlers.onRectChange.mock.calls[0][1].x).toBe(110);

    fireEvent.keyDown(dialog, { key: "ArrowUp" });
    expect(handlers.onRectChange).toHaveBeenCalledTimes(2);

    fireEvent.keyDown(dialog, { key: "Escape" });
    expect(handlers.onClose).toHaveBeenCalledWith("game-1");
  });

  it("expands with Enter", () => {
    const handlers = renderBoard();
    fireEvent.keyDown(screen.getByTestId("pip-window-game-1"), { key: "Enter" });
    expect(handlers.onExpand).toHaveBeenCalledWith("game-1");
  });
});

describe("PipWindowLayer", () => {
  const windows: PipWindowState[] = [
    { id: "pip-game-1", gameId: "game-1", rect: RECT, zIndex: 1 },
  ];

  it("renders nothing when no games are pinned", () => {
    const { container } = render(
      <PipWindowLayer
        windows={[]}
        viewport={VIEWPORT}
        onFocus={vi.fn()}
        onClose={vi.fn()}
        onExpand={vi.fn()}
        onRectChange={vi.fn()}
      />,
    );
    expect(container.firstChild).toBeNull();
  });

  it("renders a live board per pinned game", () => {
    render(
      <PipWindowLayer
        windows={windows}
        viewport={VIEWPORT}
        onFocus={vi.fn()}
        onClose={vi.fn()}
        onExpand={vi.fn()}
        onRectChange={vi.fn()}
      />,
    );

    expect(screen.getByTestId("pip-layer")).toBeInTheDocument();
    expect(screen.getByTestId("pip-window-game-1")).toBeInTheDocument();
    expect(screen.getByTestId("pip-move-game-1").textContent).toContain("1.e4");
  });
});
