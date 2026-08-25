"use client";

import React, { useState, useCallback, useRef, useEffect } from "react";

interface KeyboardMoveInputProps {
  /** Submit a SAN move. Returns true if the move was legal and accepted. */
  onSubmitMove: (san: string) => boolean;
  /** Whether the game is still in progress */
  isGameActive: boolean;
  /** Whether it is the player's turn */
  isMyTurn: boolean;
}

const PLACEHOLDER_HINTS = [
  "Type a move, e.g. e4, Nf3, O-O, Bxc6",
  "Press Enter to submit",
];

export function KeyboardMoveInput({
  onSubmitMove,
  isGameActive,
  isMyTurn,
}: KeyboardMoveInputProps) {
  const [inputValue, setInputValue] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const feedbackTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const clearFeedback = useCallback(() => {
    if (feedbackTimeoutRef.current) {
      clearTimeout(feedbackTimeoutRef.current);
      feedbackTimeoutRef.current = null;
    }
  }, []);

  useEffect(() => {
    return () => clearFeedback();
  }, [clearFeedback]);

  // Global shortcut: press Enter when input is not focused to focus the input
  useEffect(() => {
    const handleGlobalKeyDown = (e: KeyboardEvent) => {
      // Don't capture when typing in another input or modal
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;

      if (e.key === "Enter" && isGameActive && isMyTurn) {
        e.preventDefault();
        inputRef.current?.focus();
      }
    };

    document.addEventListener("keydown", handleGlobalKeyDown);
    return () => document.removeEventListener("keydown", handleGlobalKeyDown);
  }, [isGameActive, isMyTurn]);

  const handleSubmit = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      const san = inputValue.trim();
      if (!san) return;

      clearFeedback();

      const accepted = onSubmitMove(san);
      if (accepted) {
        setInputValue("");
        setSuccess(`Move submitted: ${san}`);
        feedbackTimeoutRef.current = setTimeout(() => setSuccess(null), 2000);
      } else {
        setError(`Illegal move: ${san}. Try again.`);
        feedbackTimeoutRef.current = setTimeout(() => setError(null), 3000);
      }
    },
    [inputValue, onSubmitMove, clearFeedback],
  );

  if (!isGameActive) return null;

  return (
    <form
      onSubmit={handleSubmit}
      className="w-full"
      aria-label="Keyboard move input"
    >
      <div className="flex items-center gap-2">
        <label htmlFor="keyboard-move-input" className="sr-only">
          Enter chess move in algebraic notation
        </label>
        <div className="relative flex-1">
          <input
            ref={inputRef}
            id="keyboard-move-input"
            type="text"
            value={inputValue}
            onChange={(e) => {
              setInputValue(e.target.value.toUpperCase() === e.target.value && e.target.value.length > 0 ? e.target.value : e.target.value);
              setError(null);
            }}
            placeholder={isMyTurn ? PLACEHOLDER_HINTS[0] : "Waiting for opponent..."}
            disabled={!isMyTurn}
            autoComplete="off"
            autoCorrect="off"
            autoCapitalize="off"
            spellCheck={false}
            aria-describedby="keyboard-move-hint keyboard-move-error keyboard-move-success"
            aria-invalid={!!error}
            className="w-full px-3 py-2 rounded-lg bg-gray-800/60 border border-gray-700/50 text-white text-sm font-mono placeholder:text-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500/50 focus:border-blue-500/50 disabled:opacity-50 disabled:cursor-not-allowed transition-all duration-200"
          />
          {/* Blinking cursor indicator when focused and it's the player's turn */}
          {isMyTurn && (
            <span className="absolute right-3 top-1/2 -translate-y-1/2 text-xs text-blue-400 pointer-events-none" aria-hidden="true">
              ⌨
            </span>
          )}
        </div>
        <button
          type="submit"
          disabled={!isMyTurn || !inputValue.trim()}
          aria-label="Submit move"
          className="px-4 py-2 rounded-lg bg-blue-600 hover:bg-blue-500 text-white text-sm font-medium transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed focus:outline-none focus:ring-2 focus:ring-blue-500/50"
        >
          Submit
        </button>
      </div>

      {/* Hint text */}
      <p id="keyboard-move-hint" className="mt-1 text-xs text-gray-500" aria-hidden="true">
        Press <kbd className="px-1 py-0.5 rounded bg-gray-700/60 text-gray-400 text-[10px]">Enter</kbd> to focus. Type SAN notation (e.g. <span className="font-mono">e4</span>, <span className="font-mono">Nf3</span>, <span className="font-mono">O-O</span>, <span className="font-mono">Qh5#</span>).
      </p>

      {/* Error feedback */}
      <div
        id="keyboard-move-error"
        role="alert"
        aria-live="assertive"
        aria-atomic="true"
        className="sr-only"
      >
        {error}
      </div>
      {error && (
        <p className="mt-1 text-xs text-red-400 font-medium" aria-hidden="true">
          {error}
        </p>
      )}

      {/* Success feedback */}
      <div
        id="keyboard-move-success"
        role="status"
        aria-live="polite"
        aria-atomic="true"
        className="sr-only"
      >
        {success}
      </div>
      {success && (
        <p className="mt-1 text-xs text-emerald-400" aria-hidden="true">
          {success}
        </p>
      )}
    </form>
  );
}
