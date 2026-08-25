"use client";

import React, { useState, useRef, useCallback, useId } from "react";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface KeyboardMoveInputProps {
  /**
   * Called when the user submits a valid SAN move string.
   * Return `true` if the move was accepted, `false` / undefined if illegal.
   */
  onSubmitMove: (san: string) => boolean | undefined;
  /** Whether it is currently the local player's turn. */
  isPlayerTurn: boolean;
  /** Disable the input entirely (e.g. game over). */
  disabled?: boolean;
  /** Optional Tailwind/CSS class additions for the wrapper div. */
  className?: string;
}

// ---------------------------------------------------------------------------
// SAN validation helpers
// ---------------------------------------------------------------------------

/**
 * Very permissive SAN syntax check — the real validation happens in chess.js
 * when `onSubmitMove` is called.  This just prevents obvious garbage from
 * ever reaching the game engine.
 *
 * Accepts:
 *   - Pawn moves:           e4, exd5, e8=Q, exd8=Q
 *   - Piece moves:          Nf3, Bxe5, Qh5+, Rd1#
 *   - Castling:             O-O, O-O-O  (also allows 0-0 / 0-0-0)
 *   - With check/checkmate: trailing + or #
 */
function looksLikeSan(value: string): boolean {
  const trimmed = value.trim();
  if (!trimmed) return false;

  // Castling (both O and 0 variants are common in digital notation)
  if (/^[Oo0]-[Oo0](-[Oo0])?[+#]?$/.test(trimmed)) return true;

  // Piece moves: optional piece [KQRBN], optional source disambiguation,
  // optional capture x, destination square, optional promotion, optional check
  if (/^[KQRBNa-h]?[a-h1-8]?x?[a-h][1-8](=[QRBNqrbn])?[+#]?$/.test(trimmed))
    return true;

  return false;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/**
 * KeyboardMoveInput
 *
 * A keyboard-accessible text field that accepts FIDE / standard algebraic
 * notation (SAN) move strings (e.g. "e4", "Nf3", "O-O", "exd5=Q+").
 *
 * Layout:
 *  - An "Enter move" button (or keyboard shortcut Enter//) focuses the input.
 *  - The input has proper ARIA labelling (aria-labelledby, aria-describedby, aria-invalid).
 *  - Escape cancels and returns focus to the board.
 *  - Feedback (error / success) is surfaced via an aria-live region.
 *
 * This component is intentionally unstyled beyond structure so that it can be
 * dropped into any Tailwind dark-mode layout.
 */
export function KeyboardMoveInput({
  onSubmitMove,
  isPlayerTurn,
  disabled = false,
  className = "",
}: KeyboardMoveInputProps) {
  const [value, setValue] = useState("");
  const [feedback, setFeedback] = useState<{
    message: string;
    type: "error" | "success" | "info";
  } | null>(null);
  const [isActive, setIsActive] = useState(false);

  const inputRef = useRef<HTMLInputElement>(null);
  const feedbackId = useId();
  const inputId = useId();
  const labelId = useId();

  // ── Helpers ──────────────────────────────────────────────────────────────

  const clearFeedback = useCallback(() => setFeedback(null), []);

  const showFeedback = useCallback(
    (message: string, type: "error" | "success" | "info") => {
      // Clear first so the live region re-fires for identical messages
      setFeedback(null);
      setTimeout(() => setFeedback({ message, type }), 0);
    },
    [],
  );

  const activate = useCallback(() => {
    if (disabled) return;
    setIsActive(true);
    // Defer focus slightly so the state commit renders the input first
    setTimeout(() => inputRef.current?.focus(), 0);
  }, [disabled]);

  const deactivate = useCallback(() => {
    setIsActive(false);
    setValue("");
    clearFeedback();
  }, [clearFeedback]);

  // ── Event handlers ────────────────────────────────────────────────────────

  const handleSubmit = useCallback(
    (e?: React.FormEvent) => {
      e?.preventDefault();

      const trimmed = value.trim();
      if (!trimmed) return;

      if (!isPlayerTurn) {
        showFeedback("Not your turn", "error");
        return;
      }

      if (!looksLikeSan(trimmed)) {
        showFeedback(
          `"${trimmed}" doesn't look like a valid move. Try: e4, Nf3, O-O`,
          "error",
        );
        return;
      }

      const accepted = onSubmitMove(trimmed);

      if (accepted) {
        showFeedback(`Move ${trimmed} played`, "success");
        setValue("");
        // Auto-collapse after a successful move
        setTimeout(() => deactivate(), 800);
      } else {
        showFeedback(`Illegal move: ${trimmed}`, "error");
        // Keep input active so user can correct the move
        requestAnimationFrame(() => inputRef.current?.select());
      }
    },
    [value, isPlayerTurn, onSubmitMove, showFeedback, deactivate],
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      if (e.key === "Escape") {
        e.preventDefault();
        deactivate();
      } else if (e.key === "Enter") {
        e.preventDefault();
        handleSubmit();
      }
    },
    [deactivate, handleSubmit],
  );

  /** Allow the board's global keydown to activate the input via "/" */
  const handleGlobalKeyForActivation = useCallback(
    (e: React.KeyboardEvent) => {
      if (!isActive && !disabled && (e.key === "/" || e.key === "Enter")) {
        e.preventDefault();
        activate();
      } else if (isActive && e.key === "Escape") {
        // If active, allow Escape to deactivate even when focus is elsewhere
        e.preventDefault();
        deactivate();
      }
    },
    [activate, deactivate, isActive, disabled],
  );

  // ── Render ────────────────────────────────────────────────────────────────

  const isEffectivelyDisabled = disabled || !isPlayerTurn;
  const feedbackColor =
    feedback?.type === "error"
      ? "text-red-400"
      : feedback?.type === "success"
        ? "text-emerald-400"
        : "text-gray-400";

  return (
    <div
      className={`keyboard-move-input ${className}`}
      onKeyDown={handleGlobalKeyForActivation}
    >
      {/* Activate button — visible when input is NOT active */}
      {!isActive && (
        <button
          type="button"
          onClick={activate}
          disabled={isEffectivelyDisabled}
          aria-label={
            isEffectivelyDisabled
              ? "Move input (not your turn)"
              : "Type a move in algebraic notation (press Enter or /)"
          }
          aria-haspopup="true"
          aria-expanded={isActive}
          className={[
            "w-full flex items-center justify-between gap-2 px-4 py-2.5",
            "rounded-xl border transition-all duration-200 text-sm",
            isEffectivelyDisabled
              ? "border-gray-700/30 bg-gray-800/20 text-gray-600 cursor-not-allowed opacity-50"
              : "border-gray-600/50 bg-gray-800/50 text-gray-300",
            "hover:enabled:bg-gray-700/50 hover:enabled:border-gray-500/60",
            "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-teal-500",
          ].join(" ")}
        >
          <span className="flex items-center gap-2">
            {/* Keyboard icon */}
            <svg
              aria-hidden="true"
              xmlns="http://www.w3.org/2000/svg"
              className="h-4 w-4 text-gray-400"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth={2}
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M9 12h.01M15 12h.01M9 16h.01M15 16h.01M12 12h.01M12 16h.01M5 20h14a2 2 0 002-2V6a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z"
              />
            </svg>
            Type a move
          </span>
          <kbd
            aria-hidden="true"
            className="px-1.5 py-0.5 rounded bg-gray-700/50 border border-gray-600/50 text-xs text-gray-400 font-mono"
          >
            /
          </kbd>
        </button>
      )}

      {/* Expanded input form */}
      {isActive && (
        <form
          onSubmit={handleSubmit}
          role="search"
          aria-label="Enter chess move"
          className="flex flex-col gap-1.5"
          noValidate
        >
          <div className="flex items-center gap-2">
            {/* Label (visually hidden, associated via htmlFor) */}
            <label
              id={labelId}
              htmlFor={inputId}
              className="sr-only"
            >
              Enter move in algebraic notation (e.g. e4, Nf3, O-O)
            </label>

            <input
              ref={inputRef}
              id={inputId}
              type="text"
              aria-labelledby={labelId}
              aria-describedby={feedback ? feedbackId : undefined}
              aria-invalid={feedback?.type === "error" ? "true" : "false"}
              autoComplete="off"
              autoCapitalize="none"
              spellCheck={false}
              value={value}
              onChange={(e) => {
                setValue(e.target.value);
                clearFeedback();
              }}
              onKeyDown={handleKeyDown}
              placeholder="e4, Nf3, O-O …"
              maxLength={10}
              className={[
                "flex-1 px-3 py-2 rounded-xl border text-sm font-mono bg-gray-900",
                "placeholder:text-gray-600 text-white",
                "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-teal-500",
                feedback?.type === "error"
                  ? "border-red-500/60"
                  : "border-gray-600/50",
                "transition-colors duration-150",
              ].join(" ")}
            />

            {/* Submit */}
            <button
              type="submit"
              aria-label="Submit move"
              className={[
                "px-4 py-2 rounded-xl text-sm font-semibold transition-all duration-200",
                "bg-teal-600 hover:bg-teal-500 text-white",
                "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-teal-400",
                "disabled:opacity-50 disabled:cursor-not-allowed",
              ].join(" ")}
              disabled={!value.trim()}
            >
              Move
            </button>

            {/* Cancel */}
            <button
              type="button"
              onClick={deactivate}
              aria-label="Cancel move input"
              className={[
                "px-3 py-2 rounded-xl text-sm transition-all duration-200",
                "border border-gray-600/50 bg-gray-800/50 text-gray-400 hover:text-white",
                "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-gray-500",
              ].join(" ")}
            >
              <span aria-hidden="true">✕</span>
            </button>
          </div>

          {/* Hint */}
          <p className="text-xs text-gray-500 pl-0.5">
            Type a move (e.g.{" "}
            <code className="font-mono text-gray-400">e4</code>,{" "}
            <code className="font-mono text-gray-400">Nf3</code>,{" "}
            <code className="font-mono text-gray-400">O-O</code>) then press{" "}
            <kbd className="px-1 py-0.5 rounded bg-gray-700/50 border border-gray-600/50 text-xs font-mono">
              Enter
            </kbd>{" "}
            or click Move. Press{" "}
            <kbd className="px-1 py-0.5 rounded bg-gray-700/50 border border-gray-600/50 text-xs font-mono">
              Esc
            </kbd>{" "}
            to cancel.
          </p>

          {/* Aria-live feedback region */}
          <div
            id={feedbackId}
            role="status"
            aria-live="polite"
            aria-atomic="true"
            className={`text-xs pl-0.5 min-h-[1rem] ${feedbackColor}`}
          >
            {feedback?.message ?? ""}
          </div>
        </form>
      )}
    </div>
  );
}

export default KeyboardMoveInput;
