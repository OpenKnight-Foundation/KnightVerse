import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { renderHook, act } from "@testing-library/react";
import React from "react";
import { useBoardAnnouncer } from "@/hook/useBoardAnnouncer";
import { KeyboardMoveInput } from "@/components/chess/KeyboardMoveInput";

// ─── useBoardAnnouncer tests ───────────────────────────────────────────────

describe("useBoardAnnouncer", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("starts with an empty announcement", () => {
    const { result } = renderHook(() => useBoardAnnouncer());
    expect(result.current.announcement).toBe("");
  });

  it("announces a move with correct description", () => {
    const { result } = renderHook(() => useBoardAnnouncer());

    act(() => {
      result.current.announceMove({
        color: "w",
        isCapture: false,
        isCheck: false,
        piece: "N",
        from: "g1",
        to: "f3",
      });
    });

    act(() => {
      vi.advanceTimersByTime(100);
    });

    expect(result.current.announcement).toBe(
      "White moved Knight from g1 to f3",
    );
  });

  it("announces a capture", () => {
    const { result } = renderHook(() => useBoardAnnouncer());

    act(() => {
      result.current.announceMove({
        color: "w",
        isCapture: true,
        isCheck: false,
        piece: "P",
        from: "e4",
        to: "d5",
      });
    });

    act(() => {
      vi.advanceTimersByTime(100);
    });

    expect(result.current.announcement).toContain("capturing");
  });

  it("announces check", () => {
    const { result } = renderHook(() => useBoardAnnouncer());

    act(() => {
      result.current.announceMove({
        color: "w",
        isCapture: false,
        isCheck: true,
        piece: "Q",
        from: "d1",
        to: "h5",
      });
    });

    act(() => {
      vi.advanceTimersByTime(100);
    });

    expect(result.current.announcement).toContain("checking the Black King");
  });

  it("announces black's move correctly", () => {
    const { result } = renderHook(() => useBoardAnnouncer());

    act(() => {
      result.current.announceMove({
        color: "b",
        isCapture: false,
        isCheck: false,
        piece: "P",
        from: "e7",
        to: "e5",
      });
    });

    act(() => {
      vi.advanceTimersByTime(100);
    });

    expect(result.current.announcement).toBe(
      "Black moved Pawn from e7 to e5",
    );
  });

  it("announces time alerts", () => {
    const { result } = renderHook(() => useBoardAnnouncer());

    act(() => {
      result.current.announceTimeAlert("30 seconds remaining");
    });

    act(() => {
      vi.advanceTimersByTime(100);
    });

    expect(result.current.announcement).toBe(
      "Time alert: 30 seconds remaining",
    );
  });
});

// ─── KeyboardMoveInput tests ───────────────────────────────────────────────

describe("KeyboardMoveInput", () => {
  const defaultProps = {
    onSubmitMove: vi.fn(() => true),
    isGameActive: true,
    isMyTurn: true,
    playerColor: "white" as const,
  };

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders the input field with correct placeholder when it's the player's turn", () => {
    render(<KeyboardMoveInput {...defaultProps} />);
    const input = screen.getByRole("textbox", {
      name: /enter chess move in algebraic notation/i,
    });
    expect(input).toBeInTheDocument();
    expect(input).toHaveAttribute(
      "placeholder",
      expect.stringContaining("e4"),
    );
  });

  it("renders disabled input when it's not the player's turn", () => {
    render(<KeyboardMoveInput {...defaultProps} isMyTurn={false} />);
    const input = screen.getByRole("textbox", {
      name: /enter chess move in algebraic notation/i,
    });
    expect(input).toBeDisabled();
  });

  it("does not render when game is not active", () => {
    const { container } = render(
      <KeyboardMoveInput {...defaultProps} isGameActive={false} />,
    );
    expect(container.firstChild).toBeNull();
  });

  it("submits a valid SAN move on Enter", async () => {
    const onSubmitMove = vi.fn(() => true);
    const user = userEvent.setup();

    render(<KeyboardMoveInput {...defaultProps} onSubmitMove={onSubmitMove} />);
    const input = screen.getByRole("textbox", {
      name: /enter chess move in algebraic notation/i,
    });

    await user.type(input, "e4");
    await user.keyboard("{Enter}");

    expect(onSubmitMove).toHaveBeenCalledWith("e4");
  });

  it("shows error message for illegal move", async () => {
    const onSubmitMove = vi.fn(() => false);
    const user = userEvent.setup();

    render(<KeyboardMoveInput {...defaultProps} onSubmitMove={onSubmitMove} />);
    const input = screen.getByRole("textbox", {
      name: /enter chess move in algebraic notation/i,
    });

    await user.type(input, "z9");
    await user.keyboard("{Enter}");

    expect(screen.getAllByText(/illegal move/i).length).toBeGreaterThan(0);
    expect(input).toHaveAttribute("aria-invalid", "true");
  });

  it("clears input on successful move", async () => {
    const onSubmitMove = vi.fn(() => true);
    const user = userEvent.setup();

    render(<KeyboardMoveInput {...defaultProps} onSubmitMove={onSubmitMove} />);
    const input = screen.getByRole("textbox", {
      name: /enter chess move in algebraic notation/i,
    });

    await user.type(input, "Nf3");
    await user.keyboard("{Enter}");

    expect(input).toHaveValue("");
  });

  it("shows success message on valid move", async () => {
    const onSubmitMove = vi.fn(() => true);
    const user = userEvent.setup();

    render(<KeyboardMoveInput {...defaultProps} onSubmitMove={onSubmitMove} />);
    const input = screen.getByRole("textbox", {
      name: /enter chess move in algebraic notation/i,
    });

    await user.type(input, "e4");
    await user.keyboard("{Enter}");

    expect(screen.getAllByText(/move submitted/i).length).toBeGreaterThan(0);
  });

  it("has an aria-live error region for screen readers", () => {
    render(<KeyboardMoveInput {...defaultProps} />);
    const liveRegion = screen.getByRole("alert", { hidden: true });
    expect(liveRegion).toHaveAttribute("aria-live", "assertive");
  });

  it("has a submit button with accessible label", () => {
    render(<KeyboardMoveInput {...defaultProps} />);
    const button = screen.getByRole("button", { name: /submit move/i });
    expect(button).toBeInTheDocument();
  });

  it("submit button is disabled when there is no input", () => {
    render(<KeyboardMoveInput {...defaultProps} />);
    const button = screen.getByRole("button", { name: /submit move/i });
    expect(button).toBeDisabled();
  });

  it("submit button is disabled when it is not the player's turn", () => {
    render(<KeyboardMoveInput {...defaultProps} isMyTurn={false} />);
    const button = screen.getByRole("button", { name: /submit move/i });
    expect(button).toBeDisabled();
  });
});

// ─── axe-core accessibility audit ──────────────────────────────────────────

describe("KeyboardMoveInput accessibility audit", () => {
  it("has no axe-core violations", async () => {
    const axe = (await import("axe-core")).default;

    const { container } = render(
      <KeyboardMoveInput
        onSubmitMove={() => true}
        isGameActive={true}
        isMyTurn={true}
        
      />,
    );

    const results = await axe.run(container);

    expect(results.violations).toEqual([]);
  });

  it("has no axe-core violations when disabled", async () => {
    const axe = (await import("axe-core")).default;

    const { container } = render(
      <KeyboardMoveInput
        onSubmitMove={() => true}
        isGameActive={true}
        isMyTurn={false}
        
      />,
    );

    const results = await axe.run(container);

    expect(results.violations).toEqual([]);
  });
});
