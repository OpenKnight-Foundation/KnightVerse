
"use client";

import React from "react";
import { FaTrophy, FaGamepad } from "react-icons/fa";
import { useFocusTrap } from "@/hook/useFocusTrap";

interface MatchmakingModalProps {
  isOpen: boolean;
  onClose: () => void;
  onConfirm: (type: "Rated" | "Casual") => void;
}

/**
 * Matchmaking Modal
 *
 * Allows players to select a matchmaking mode before starting a game.
 * Supports rated matches, which affect ELO points, and casual matches,
 * which allow players to practice without affecting their rating.
 *
 * Accessibility features:
 * - Traps keyboard focus within the modal while it is open.
 * - Closes the modal when the Escape key is pressed.
 * - Exposes dialog semantics and an accessible title to assistive technology.
 * - Provides visible keyboard focus indicators for interactive elements.
 *
 * @param isOpen - Determines whether the modal is displayed.
 * @param onClose - Callback invoked when the modal is dismissed.
 * @param onConfirm - Callback invoked with the selected match type.
 */
export function MatchmakingModal({
  isOpen,
  onClose,
  onConfirm,
}: MatchmakingModalProps) {
  // Keep keyboard focus inside the open modal and delegate Escape handling
  // to the reusable focus-trap hook.
  const trapRef = useFocusTrap({ active: isOpen, onEscape: onClose });

  // Do not render the modal or its overlay when it is closed.
  if (!isOpen) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/80 p-4 backdrop-blur-md animate-overlay-in"
      role="presentation"
    >
      <div
        ref={trapRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="matchmaking-modal-title"
        className="w-full max-w-md rounded-3xl border border-teal-500/30 bg-gray-900/90 p-8 shadow-[0_0_50px_rgba(20,184,166,0.15)] animate-modal-in"
      >
        <h2
          id="matchmaking-modal-title"
          className="mb-2 bg-gradient-to-r from-teal-400 to-blue-500 bg-clip-text text-center text-3xl font-extrabold tracking-tight text-transparent"
        >
          Select Match Type
        </h2>

        <p className="mb-8 text-center text-sm font-medium uppercase tracking-widest text-gray-400">
          Choose your competitive level
        </p>

        {/* Present the available matchmaking modes as a labeled group. */}
        <div className="grid gap-4" role="group" aria-label="Match type options">
          {/* Rated matches affect the player's ELO rating. */}
          <button
            type="button"
            onClick={() => onConfirm("Rated")}
            className="group relative flex items-center gap-6 rounded-2xl border border-teal-500/20 bg-gradient-to-br from-teal-500/10 to-blue-500/10 p-5 text-left transition-all duration-300 hover:border-teal-400/50 hover:bg-teal-500/20 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-teal-400"
          >
            <div
              className="rounded-xl bg-teal-500/20 p-4 transition-transform duration-300 group-hover:scale-110"
              aria-hidden="true"
            >
              <FaTrophy className="text-2xl text-teal-400" />
            </div>

            <div>
              <h3 className="text-xl font-bold text-white transition-colors group-hover:text-teal-300">
                Rated Match
              </h3>
              <p className="text-sm text-gray-400">
                Win or lose ELO points
              </p>
            </div>
          </button>

          {/* Casual matches provide practice without ELO rating changes. */}
          <button
            type="button"
            onClick={() => onConfirm("Casual")}
            className="group relative flex items-center gap-6 rounded-2xl border border-gray-700/50 bg-gradient-to-br from-gray-800/50 to-gray-700/50 p-5 text-left transition-all duration-300 hover:border-blue-400/50 hover:bg-blue-500/10 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400"
          >
            <div
              className="rounded-xl bg-blue-500/20 p-4 transition-transform duration-300 group-hover:scale-110"
              aria-hidden="true"
            >
              <FaGamepad className="text-2xl text-blue-400" />
            </div>

            <div>
              <h3 className="text-xl font-bold text-white transition-colors group-hover:text-blue-300">
                Casual Match
              </h3>
              <p className="text-sm text-gray-400">
                Practice without ELO stakes
              </p>
            </div>
          </button>
        </div>

        {/* Allow players to dismiss the modal without selecting a match type. */}
        <button
          type="button"
          onClick={onClose}
          className="mt-8 w-full rounded-xl py-3 text-sm font-medium text-gray-500 transition-colors hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-gray-500"
        >
          Cancel
        </button>
      </div>

      {/* Define entrance animations for the overlay and modal content. */}
      <style jsx>{`
        @keyframes overlay-in {
          from {
            opacity: 0;
          }
          to {
            opacity: 1;
          }
        }

        @keyframes modal-in {
          from {
            opacity: 0;
            transform: scale(0.9) translateY(20px);
          }
          to {
            opacity: 1;
            transform: scale(1) translateY(0);
          }
        }

        .animate-overlay-in {
          animation: overlay-in 0.3s ease-out;
        }

        .animate-modal-in {
          animation: modal-in 0.4s cubic-bezier(0.16, 1, 0.3, 1);
        }

        @media (prefers-reduced-motion: reduce) {
          .animate-overlay-in,
          .animate-modal-in {
            animation: none;
          }
        }
      `}</style>
    </div>
  );
}
