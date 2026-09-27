/**
 * Tests for TournamentBracketViewer — FE-56 (#1115)
 *
 * Covers:
 * - Tournament header (name, status, format)
 * - Champion banner when tournament is completed
 * - Bracket grid rendered for elimination formats
 * - Swiss standings rendered for Swiss format
 * - Spectate button calls onSpectate with match ID
 * - Participants list
 */

import React from "react";
import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import {
  TournamentBracketViewer,
  type TournamentBracket,
} from "../TournamentBracketViewer";

const PARTICIPANTS = [
  { id: "p1", wallet_address: "0xA", display_name: "Alice", elo: 1800, seed: 1 },
  { id: "p2", wallet_address: "0xB", display_name: "Bob", elo: 1750, seed: 2 },
  { id: "p3", wallet_address: "0xC", display_name: "Carol", elo: 1700, seed: 3 },
  { id: "p4", wallet_address: "0xD", display_name: "Dave", elo: 1650, seed: 4 },
];

const BASE_BRACKET: TournamentBracket = {
  id: "t1",
  name: "Test Open",
  format: "SingleElimination",
  status: "InProgress",
  participants: PARTICIPANTS,
  matches: [
    {
      id: "m1",
      round: 1,
      match_number: 1,
      player1_id: "p1",
      player2_id: "p2",
      winner_id: "p1",
      status: "Checkmate",
      scheduled_at: null,
      completed_at: null,
    },
    {
      id: "m2",
      round: 1,
      match_number: 2,
      player1_id: "p3",
      player2_id: "p4",
      winner_id: null,
      status: "Ongoing",
      scheduled_at: null,
      completed_at: null,
    },
    {
      id: "m3",
      round: 2,
      match_number: 1,
      player1_id: "p1",
      player2_id: null,
      winner_id: null,
      status: "Upcoming",
      scheduled_at: null,
      completed_at: null,
    },
  ],
  total_rounds: 2,
  winner_id: null,
  created_at: "2026-01-01T00:00:00Z",
  started_at: "2026-01-02T00:00:00Z",
  completed_at: null,
};

describe("TournamentBracketViewer (FE-56 / #1115)", () => {
  it("renders tournament name and status", () => {
    render(<TournamentBracketViewer bracket={BASE_BRACKET} />);
    expect(screen.getByText("Test Open")).toBeInTheDocument();
    expect(screen.getByText("InProgress")).toBeInTheDocument();
  });

  it("renders format label", () => {
    render(<TournamentBracketViewer bracket={BASE_BRACKET} />);
    expect(screen.getByText("Single Elimination")).toBeInTheDocument();
  });

  it("renders match status badges for elimination format", () => {
    render(<TournamentBracketViewer bracket={BASE_BRACKET} />);
    expect(screen.getAllByText("Checkmate").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Ongoing").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Upcoming").length).toBeGreaterThan(0);
  });

  it("renders spectate button for ongoing match", () => {
    const onSpectate = vi.fn();
    render(
      <TournamentBracketViewer bracket={BASE_BRACKET} onSpectate={onSpectate} />
    );
    const watchBtn = screen.getByRole("button", { name: /spectate match m2/i });
    fireEvent.click(watchBtn);
    expect(onSpectate).toHaveBeenCalledWith("m2");
  });

  it("renders champion banner when tournament is completed", () => {
    const completed: TournamentBracket = {
      ...BASE_BRACKET,
      status: "Completed",
      winner_id: "p1",
    };
    render(<TournamentBracketViewer bracket={completed} />);
    expect(screen.getByText("Champion")).toBeInTheDocument();
    expect(screen.getAllByText("Alice").length).toBeGreaterThan(0);
  });

  it("does not render champion banner when tournament is in progress", () => {
    render(<TournamentBracketViewer bracket={BASE_BRACKET} />);
    expect(screen.queryByText("Champion")).not.toBeInTheDocument();
  });

  it("renders all participants in the participants list", () => {
    render(<TournamentBracketViewer bracket={BASE_BRACKET} />);
    for (const p of PARTICIPANTS) {
      expect(screen.getByText(p.display_name)).toBeInTheDocument();
    }
  });

  it("renders Swiss standings for Swiss format", () => {
    const swiss: TournamentBracket = {
      ...BASE_BRACKET,
      format: "Swiss",
    };
    render(<TournamentBracketViewer bracket={swiss} />);
    // Swiss standings table shows column headers
    expect(screen.getByText("Rank")).toBeInTheDocument();
    expect(screen.getByText("Player")).toBeInTheDocument();
    expect(screen.getByText("Wins")).toBeInTheDocument();
  });

  it("renders participant count", () => {
    render(<TournamentBracketViewer bracket={BASE_BRACKET} />);
    expect(screen.getByText(/4 players/i)).toBeInTheDocument();
  });

  it("highlights the current user's match", () => {
    render(
      <TournamentBracketViewer
        bracket={BASE_BRACKET}
        currentUserId="p3"
      />
    );
    expect(screen.getByText("Your match")).toBeInTheDocument();
  });

  it("renders the data-testid attribute", () => {
    render(<TournamentBracketViewer bracket={BASE_BRACKET} />);
    expect(
      screen.getByTestId("tournament-bracket-viewer")
    ).toBeInTheDocument();
  });
});
