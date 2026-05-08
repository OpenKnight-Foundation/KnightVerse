"use client";

import React, { useEffect, useRef } from "react";
import { FaTrophy, FaGamepad } from "react-icons/fa";

interface MatchmakingModalProps {
  isOpen: boolean;
  onClose: () => void;
  onConfirm: (type: "Rated" | "Casual") => void;
}

export function MatchmakingModal({
  isOpen,
  onClose,
  onConfirm,
}: MatchmakingModalProps) {
  const firstButtonRef = useRef<HTMLButtonElement>(null);
  const cancelButtonRef = useRef<HTMLButtonElement>(null);

  // Close on Escape and trap focus inside the modal
  useEffect(() => {
    if (!isOpen) return;

    firstButtonRef.current?.focus();

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
        return;
      }

      // Focus trap: cycle Tab between modal focusable elements
      if (e.key === "Tab") {
        const focusable = [firstButtonRef.current, cancelButtonRef.current].filter(
          Boolean
        ) as HTMLButtonElement[];
        const first = focusable[0];
        const last = focusable[focusable.length - 1];

        if (e.shiftKey) {
          if (document.activeElement === first) {
            e.preventDefault();
            last.focus();
          }
        } else {
          if (document.activeElement === last) {
            e.preventDefault();
            first.focus();
          }
        }
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  return (
    <div
      className="fixed inset-0 bg-black/80 backdrop-blur-md z-50 flex items-center justify-center p-4 animate-overlay-in"
      role="dialog"
      aria-modal="true"
      aria-labelledby="matchmaking-modal-title"
    >
      <div className="bg-gray-900/90 border border-teal-500/30 rounded-3xl p-8 max-w-md w-full shadow-[0_0_50px_rgba(20,184,166,0.15)] animate-modal-in">
        <h2
          id="matchmaking-modal-title"
          className="text-3xl font-extrabold text-white mb-2 tracking-tight text-center bg-gradient-to-r from-teal-400 to-blue-500 bg-clip-text text-transparent"
        >
          Select Match Type
        </h2>
        <p className="text-gray-400 text-center mb-8 text-sm uppercase tracking-widest font-medium">
          Choose your competitive level
        </p>

        <div className="grid gap-4">
          <button
            ref={firstButtonRef}
            onClick={() => onConfirm("Rated")}
            className="group relative flex items-center gap-6 p-5 rounded-2xl bg-gradient-to-br from-teal-500/10 to-blue-500/10 border border-teal-500/20 hover:border-teal-400/50 hover:bg-teal-500/20 transition-all duration-300 text-left"
          >
            <div className="bg-teal-500/20 p-4 rounded-xl group-hover:scale-110 transition-transform duration-300">
              <FaTrophy className="text-2xl text-teal-400" />
            </div>
            <div>
              <h3 className="text-xl font-bold text-white group-hover:text-teal-300 transition-colors">
                Rated Match
              </h3>
              <p className="text-gray-400 text-sm">Win or lose ELO points</p>
            </div>
          </button>

          <button
            onClick={() => onConfirm("Casual")}
            className="group relative flex items-center gap-6 p-5 rounded-2xl bg-gradient-to-br from-gray-800/50 to-gray-700/50 border border-gray-700/50 hover:border-blue-400/50 hover:bg-blue-500/10 transition-all duration-300 text-left"
          >
            <div className="bg-blue-500/20 p-4 rounded-xl group-hover:scale-110 transition-transform duration-300">
              <FaGamepad className="text-2xl text-blue-400" />
            </div>
            <div>
              <h3 className="text-xl font-bold text-white group-hover:text-blue-300 transition-colors">
                Casual Match
              </h3>
              <p className="text-gray-400 text-sm">Practice without ELO stakes</p>
            </div>
          </button>
        </div>

        <button
          ref={cancelButtonRef}
          onClick={onClose}
          className="mt-8 w-full py-3 rounded-xl text-gray-500 hover:text-white transition-colors font-medium text-sm"
        >
          Cancel
        </button>
      </div>

      <style jsx>{`
        @keyframes overlay-in {
          from { opacity: 0; }
          to { opacity: 1; }
        }
        @keyframes modal-in {
          from { opacity: 0; transform: scale(0.9) translateY(20px); }
          to { opacity: 1; transform: scale(1) translateY(0); }
        }
        .animate-overlay-in { animation: overlay-in 0.3s ease-out; }
        .animate-modal-in { animation: modal-in 0.4s cubic-bezier(0.16, 1, 0.3, 1); }
      `}</style>
    </div>
  );
}
