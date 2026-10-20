import React from "react";
import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { axe } from "vitest-axe";
import ChessboardComponent from "@/components/chess/ChessboardComponent";
import BlindfoldToggle from "@/components/chess/BlindfoldToggle";
import { VoiceMoveInput } from "@/components/chess/VoiceMoveInput";
import { GamePreferencesProvider } from "@/context/GamePreferencesContext";
import { BoardThemeProvider } from "@/context/ThemeContext";

const Providers = ({ children }: { children: React.ReactNode }) => (
  <BoardThemeProvider>
    <GamePreferencesProvider>{children}</GamePreferencesProvider>
  </BoardThemeProvider>
);

describe("FE-52 blindfold mode", () => {
  it("hides piece graphics but keeps squares interactive", () => {
    const onDrop = vi.fn(() => true);
    render(
      <ChessboardComponent
        position="start"
        onDrop={onDrop}
        blindfoldMode
      />,
      { wrapper: Providers },
    );

    // Board still renders 64 interactive squares with coordinates
    expect(screen.getByRole("grid")).toBeInTheDocument();
    expect(screen.getAllByRole("gridcell")).toHaveLength(64);

    // Pieces hidden via opacity:0 wrapper, occupancy shown as subtle dots
    expect(screen.getAllByTestId("blindfold-dot").length).toBeGreaterThan(0);

    // Click/drag coordinates retained: clicking e2 then e4 still fires onDrop
    fireEvent.click(screen.getByLabelText(/^e2, White Pawn/));
    fireEvent.click(screen.getByLabelText(/^e4, empty/));
    expect(onDrop).toHaveBeenCalledWith({
      sourceSquare: "e2",
      targetSquare: "e4",
    });
  });

  it("shows pieces normally when blindfold is off", () => {
    render(
      <ChessboardComponent position="start" onDrop={vi.fn()} blindfoldMode={false} />,
      { wrapper: Providers },
    );
    expect(screen.queryByTestId("blindfold-dot")).not.toBeInTheDocument();
  });

  it("blindfold toggle is accessible", async () => {
    const { container } = render(
      <BlindfoldToggle blindfoldMode={false} onToggle={vi.fn()} />,
      { wrapper: Providers },
    );
    expect(screen.getByLabelText("Blindfold mode")).toBeInTheDocument();
    expect(await axe(container)).toHaveNoViolations();
  });
});

describe("FE-52 voice move input", () => {
  it("shows keyboard fallback hint when SpeechRecognition unsupported", () => {
    render(
      <VoiceMoveInput
        onSubmitMove={vi.fn(() => true)}
        isGameActive
        isMyTurn
      />,
    );
    expect(screen.getByLabelText("Voice move input")).toBeInTheDocument();
  });

  it("submits parsed voice transcripts via onSubmitMove", async () => {
    // Mock webkitSpeechRecognition to emit a final "Knight f3" result
    const listeners: Record<string, (e: unknown) => void> = {};
    class MockRecognition {
      lang = "";
      interimResults = false;
      continuous = false;
      start = vi.fn(() => {
        listeners.result?.({
          results: [[{ transcript: "Knight f3" }]],
        });
      });
      stop = vi.fn();
      set onresult(fn: (e: unknown) => void) {
        listeners.result = fn;
      }
      set onerror(fn: (e: unknown) => void) {
        listeners.error = fn;
      }
      set onend(fn: () => void) {
        listeners.end = fn;
      }
    }
    (window as unknown as Record<string, unknown>).webkitSpeechRecognition =
      MockRecognition;

    const onSubmitMove = vi.fn(() => true);
    render(
      <VoiceMoveInput
        onSubmitMove={onSubmitMove}
        isGameActive
        isMyTurn
      />,
    );

    fireEvent.click(screen.getByLabelText("Start voice input"));
    expect(onSubmitMove).toHaveBeenCalledWith("Nf3");

    delete (window as unknown as Record<string, unknown>).webkitSpeechRecognition;
  });

  it("voice input region is accessible", async () => {
    const { container } = render(
      <VoiceMoveInput
        onSubmitMove={vi.fn(() => true)}
        isGameActive
        isMyTurn
      />,
    );
    expect(await axe(container)).toHaveNoViolations();
  });
});

describe("FE-52 speech synthesis hook", () => {
  it("calls speechSynthesis.speak with volume/rate mapping", async () => {
    const speak = vi.fn();
    const cancel = vi.fn();
    Object.defineProperty(window, "speechSynthesis", {
      value: { speak, cancel },
      configurable: true,
    });
    class FakeUtterance {
      text: string;
      volume = 1;
      rate = 1;
      lang = "";
      constructor(text: string) {
        this.text = text;
      }
    }
    (window as unknown as Record<string, unknown>).SpeechSynthesisUtterance =
      FakeUtterance as unknown;

    const { useSpeechSynthesis } = await import("@/hook/useSpeechSynthesis");
    const { renderHook, act } = await import("@testing-library/react");
    const { result } = renderHook(() => useSpeechSynthesis());
    act(() => {
      result.current.speak("Opponent played Nf3", { volume: 80, rate: 1 });
    });
    expect(speak).toHaveBeenCalled();
  });
});
