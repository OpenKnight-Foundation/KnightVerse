"use client";

import React from "react";
import { useMatchmakingContext, AiPersonality } from "@/context/matchmakingContext";
import { getChessVariantById } from "@/lib/chessVariants";

interface AiPersonalityModalProps {
  isOpen: boolean;
  onClose: () => void;
  onConfirm: () => void;
}

interface PersonalityOption {
  id: AiPersonality;
  label: string;
  description: string;
  gradient: string;
  borderColor: string;
  iconBg: string;
  icon: React.ReactNode;
}

const PERSONALITIES: PersonalityOption[] = [
  {
    id: "aggressive",
    label: "Aggressive",
    description:
      "Prioritizes constant attack and tactical sacrifices to seize initiative and pressure the opponent relentlessly.",
    gradient: "from-red-500 to-orange-600",
    borderColor: "border-red-500",
    iconBg: "bg-red-500/20",
    icon: (
      <svg
        xmlns="http://www.w3.org/2000/svg"
        className="w-8 h-8 text-red-400"
        fill="none"
        viewBox="0 0 24 24"
        stroke="currentColor"
        strokeWidth={2}
      >
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          d="M13 10V3L4 14h7v7l9-11h-7z"
        />
      </svg>
    ),
  },
  {
    id: "defensive",
    label: "Defensive",
    description:
      "Fortifies position, minimises risk, and waits for the opponent to overextend before launching precise counter-attacks.",
    gradient: "from-teal-500 to-blue-700",
    borderColor: "border-teal-500",
    iconBg: "bg-teal-500/20",
    icon: (
      <svg
        xmlns="http://www.w3.org/2000/svg"
        className="w-8 h-8 text-teal-400"
        fill="none"
        viewBox="0 0 24 24"
        stroke="currentColor"
        strokeWidth={2}
      >
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z"
        />
      </svg>
    ),
  },
  {
    id: "sacrificial",
    label: "Sacrificial",
    description:
      "Willingly exchanges material for long-term positional dominance, opening lines and creating unstoppable strategic advantages.",
    gradient: "from-yellow-500 to-amber-600",
    borderColor: "border-yellow-500",
    iconBg: "bg-yellow-500/20",
    icon: (
      <svg
        xmlns="http://www.w3.org/2000/svg"
        className="w-8 h-8 text-yellow-400"
        fill="none"
        viewBox="0 0 24 24"
        stroke="currentColor"
        strokeWidth={2}
      >
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"
        />
      </svg>
    ),
  },
];

export function AiPersonalityModal({
  isOpen,
  onClose,
  onConfirm,
}: AiPersonalityModalProps) {
  const { aiPersonality, setAiPersonality, chessVariant } =
    useMatchmakingContext();
  const selectedVariant = getChessVariantById(chessVariant);

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      <div
        className="fixed inset-0 bg-black/70 backdrop-blur-sm"
        onClick={onClose}
      />

      {/* Modal panel */}
      <div className="relative z-10 w-full max-w-lg mx-4 bg-gray-900 rounded-2xl border border-gray-700 shadow-2xl p-6 space-y-6">
        {/* Close button */}
        <button
          onClick={onClose}
          className="absolute top-4 right-4 text-gray-400 hover:text-white transition-colors duration-200"
          aria-label="Close personality selector"
        >
          <svg
            xmlns="http://www.w3.org/2000/svg"
            className="h-6 w-6"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            strokeWidth={2}
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M6 18L18 6M6 6l12 12"
            />
          </svg>
        </button>

        {/* Header */}
        <div className="text-center space-y-1">
          <h2 className="text-2xl font-bold text-white tracking-wide">
            Finalize Match Setup
          </h2>
          <p className="text-gray-400 text-sm">
            Lock in your preferred chess format and AI co-pilot before matchmaking starts.
          </p>
        </div>

        <div className="rounded-2xl border border-cyan-400/20 bg-cyan-500/5 p-4">
          <div className="flex items-center justify-between gap-4">
            <div>
              <p className="text-[11px] font-semibold uppercase tracking-[0.28em] text-cyan-300/80">
                Selected Variant
              </p>
              <h3 className="mt-1 text-lg font-semibold text-white">
                {selectedVariant.label}
              </h3>
              <p className="mt-1 text-sm text-gray-300">
                {selectedVariant.description}
              </p>
            </div>
            <span className="rounded-full border border-cyan-400/30 bg-cyan-500/10 px-3 py-1 text-[11px] font-semibold uppercase tracking-[0.24em] text-cyan-100">
              {selectedVariant.averageGameTime}
            </span>
          </div>
        </div>

        {/* Personality cards */}
        <div className="space-y-3">
          {PERSONALITIES.map((option) => {
            const isSelected = aiPersonality === option.id;
            return (
              <button
                key={option.id}
                onClick={() => setAiPersonality(option.id)}
                className={`personality-card w-full flex items-center gap-4 p-4 rounded-xl border-2 text-left transition-all duration-250
                  ${
                    isSelected
                      ? `${option.borderColor} bg-gradient-to-r ${option.gradient} bg-opacity-10 selected`
                      : "border-gray-700 bg-gray-800/50 hover:border-gray-500"
                  }`}
              >
                <div className={`${option.iconBg} p-3 rounded-xl flex-shrink-0`}>
                  {option.icon}
                </div>
                <div className="flex-1 min-w-0">
                  <h3 className="text-base font-bold text-white mb-0.5">
                    {option.label}
                  </h3>
                  <p className="text-gray-300 text-xs leading-relaxed">
                    {option.description}
                  </p>
                </div>
                {/* Selection indicator */}
                <div
                  className={`flex-shrink-0 w-5 h-5 rounded-full border-2 flex items-center justify-center transition-all duration-200
                    ${isSelected ? `${option.borderColor} bg-transparent` : "border-gray-600"}`}
                >
                  {isSelected && (
                    <div className="w-2.5 h-2.5 rounded-full bg-white" />
                  )}
                </div>
              </button>
            );
          })}
        </div>

        {/* Confirm button */}
        <button
          onClick={onConfirm}
          className="confirm-btn w-full py-3 rounded-xl font-bold text-white text-sm uppercase tracking-widest bg-gradient-to-r from-teal-500 to-blue-700 hover:from-teal-600 hover:to-blue-800 transition-all duration-300 hover:scale-[1.02] active:scale-[0.98]"
        >
          Confirm &amp; Find Match
        </button>
      </div>

      <style jsx>{`
        .personality-card.selected {
          box-shadow: 0 0 0 1px rgba(45, 212, 191, 0.4),
            0 4px 20px rgba(45, 212, 191, 0.15);
        }
        .personality-card:hover:not(.selected) {
          transform: translateY(-1px);
        }
        .confirm-btn {
          box-shadow: 0 4px 15px rgba(20, 184, 166, 0.3);
        }
        .confirm-btn:hover {
          box-shadow: 0 6px 20px rgba(20, 184, 166, 0.5);
        }
      `}</style>
    </div>
  );
}
