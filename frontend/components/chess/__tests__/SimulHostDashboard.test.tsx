/**
 * Tests for SimulHostDashboard — FE-55 (#1114)
 *
 * Covers:
 * - Renders with valid game count (2–20)
 * - Throws when fewer than 2 games are provided
 * - Clock overview stat cards
 * - Keyboard navigation (Space cycles to next host-turn board)
 * - Spectator mode toggle
 * - Result ticker after game completion
 * - Board grid data-testid
 */

import React from "react";
import { render, screen, fireEvent, act } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  SimulHostDashboard,
  type SimulGame,
} from "../SimulHostDashboard";

// ── Fixtures ──────────────────────────────────────────────────────────────────

function makeGame(overrides: Partial<SimulGame> & { id: string; boardIndex: number }): SimulGame {
  return {
    opponentName: `Opponent_${overrides.id}`,
    opponentElo: 1500,
    fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
    status: "host_turn",
    hostTimeRemaining: 300,
    opponentTimeRemaining: 300,
    moveCount: 0,
    ...overrides,
  };
}

const TWO_GAMES: SimulGame[] = [
  makeGame({ id: "g1", boardIndex: 0, status: "host_turn" }),
  makeGame({ id: "g2", boardIndex: 1, status: "opponent_turn" }),
];

const FOUR_GAMES: SimulGame[] = [
  makeGame({ id: "g1", boardIndex: 0, status: "host_turn" }),
  makeGame({ id: "g2", boardIndex: 1, status: "opponent_turn" }),
  makeGame({ id: "g3", boardIndex: 2, status: "host_turn" }),
  makeGame({ id: "g4", boardIndex: 3, status: "checkmate_host" }),
];

// ── Tests ──────────────────────────────────────────────────────────────────────

describe("SimulHostDashboard (FE-55 / #1114)", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders the dashboard with 2 games", () => {
    render(<SimulHostDashboard initialGames={TWO_GAMES} hostName="GM Test" />);
    expect(screen.getByTestId("simul-host-dashboard")).toBeInTheDocument();
    expect(screen.getByText(/GM Test/)).toBeInTheDocument();
    expect(screen.getByText(/2 boards/)).toBeInTheDocument();
  });

  it("throws when fewer than 2 games are provided", () => {
    const oneGame = [makeGame({ id: "g1", boardIndex: 0 })];
    // Suppress error output
    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    expect(() =>
      render(<SimulHostDashboard initialGames={oneGame} />)
    ).toThrow(/at least 2 games/i);
    consoleSpy.mockRestore();
  });

  it("renders the board grid", () => {
    render(<SimulHostDashboard initialGames={FOUR_GAMES} />);
    expect(screen.getByTestId("simul-board-grid")).toBeInTheDocument();
    // Should show 4 mini-tiles — each has an aria-label with board number
    expect(screen.getByLabelText(/Board 1:/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/Board 4:/i)).toBeInTheDocument();
  });

  it("shows correct host-turn count in the overview", () => {
    render(<SimulHostDashboard initialGames={FOUR_GAMES} />);
    // g1 and g3 have status host_turn → count = 2
    const turns = screen.getAllByText("2");
    // The "Your turn" card shows 2
    expect(turns.length).toBeGreaterThan(0);
  });

  it("shows result in result ticker for completed game", () => {
    render(<SimulHostDashboard initialGames={FOUR_GAMES} />);
    // g4 is checkmate_host → "1–0"
    expect(screen.getByText("1–0")).toBeInTheDocument();
  });

  it("toggles spectator mode when the button is clicked", () => {
    const onToggle = vi.fn();
    render(
      <SimulHostDashboard
        initialGames={TWO_GAMES}
        onSpectatorToggle={onToggle}
      />
    );

    const toggleBtn = screen.getByRole("button", { name: /spectator/i });
    fireEvent.click(toggleBtn);

    expect(onToggle).toHaveBeenCalledWith(true);
    expect(screen.getByRole("button", { name: /spectating/i })).toBeInTheDocument();
  });

  it("focuses a board tile when clicked", () => {
    render(<SimulHostDashboard initialGames={TWO_GAMES} />);
    const board2 = screen.getByLabelText(/Board 2:/i);
    fireEvent.click(board2);
    // The focused board shows the opponent's name in the panel heading
    expect(screen.getByText(/Board #2/i)).toBeInTheDocument();
  });

  it("cycles to the next active board on Space keydown", () => {
    render(<SimulHostDashboard initialGames={FOUR_GAMES} />);
    // Initial focus = g1 (host_turn)
    // Press Space → should move to g3 (next host_turn board)
    act(() => {
      fireEvent.keyDown(window, { key: " " });
    });
    // g3 is now focused — focused board panel shows Board #3
    expect(screen.getByText(/Board #3/i)).toBeInTheDocument();
  });

  it("ticks the clock every second", () => {
    render(<SimulHostDashboard initialGames={TWO_GAMES} />);
    // g1 starts at 5:00 (300s)
    expect(screen.getAllByText("5:00").length).toBeGreaterThan(0);

    act(() => {
      vi.advanceTimersByTime(3000); // 3 seconds
    });

    // g1 (host_turn) should now show 4:57
    expect(screen.getAllByText("4:57").length).toBeGreaterThan(0);
  });

  it("shows the simul-complete banner when all games are finished", () => {
    const allDone: SimulGame[] = [
      makeGame({ id: "g1", boardIndex: 0, status: "checkmate_host" }),
      makeGame({ id: "g2", boardIndex: 1, status: "draw" }),
    ];
    render(<SimulHostDashboard initialGames={allDone} />);
    expect(screen.getByText(/Simul Complete/i)).toBeInTheDocument();
  });
});
