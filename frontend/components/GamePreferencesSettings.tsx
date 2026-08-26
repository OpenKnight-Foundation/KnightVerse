"use client";

import React from "react";
import {
  useGamePreferences,
  PieceInputMethod,
  AutoQueenMode,
  LegalMoveDots,
  BoardCoordinates,
} from "@/context/GamePreferencesContext";

interface Option<T extends string> {
  id: T;
  label: string;
  description?: string;
}

function RadioGroup<T extends string>({
  label,
  value,
  options,
  onChange,
  tooltip,
}: {
  label: string;
  value: T;
  options: Option<T>[];
  onChange: (value: T) => void;
  tooltip?: string;
}) {
  return (
    <div className="mb-6">
      <div className="flex items-center gap-2 mb-3">
        <h3 className="text-lg font-semibold text-gray-300">{label}</h3>
        {tooltip && (
          <span
            className="group relative inline-flex items-center justify-center w-5 h-5 rounded-full bg-gray-700 text-gray-300 text-xs cursor-help"
            aria-label={tooltip}
          >
            ?
            <span className="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 hidden group-hover:block w-56 p-2 text-xs text-gray-200 bg-gray-800 border border-gray-700 rounded-lg shadow-lg z-10">
              {tooltip}
            </span>
          </span>
        )}
      </div>
      <div className="flex flex-col space-y-2">
        {options.map((option) => (
          <button
            key={option.id}
            type="button"
            role="radio"
            aria-checked={value === option.id}
            onClick={() => onChange(option.id)}
            className={`px-4 py-3 text-left rounded-lg transition-all duration-300 flex items-center justify-between ${
              value === option.id
                ? "bg-gradient-to-r from-teal-600 to-blue-700 text-white font-semibold shadow-md"
                : "bg-gray-800 hover:bg-gray-700 text-gray-300 border border-gray-700"
            }`}
          >
            <span>
              <span className="block">{option.label}</span>
              {option.description && (
                <span className="block text-xs text-gray-400 mt-0.5">
                  {option.description}
                </span>
              )}
            </span>
            {value === option.id && (
              <svg
                className="w-5 h-5"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M5 13l4 4L19 7"
                />
              </svg>
            )}
          </button>
        ))}
      </div>
    </div>
  );
}

function Toggle({
  label,
  checked,
  onChange,
  tooltip,
}: {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  tooltip?: string;
}) {
  return (
    <div className="flex items-center justify-between py-3">
      <div className="flex items-center gap-2">
        <span className="text-gray-300">{label}</span>
        {tooltip && (
          <span
            className="group relative inline-flex items-center justify-center w-5 h-5 rounded-full bg-gray-700 text-gray-300 text-xs cursor-help"
            aria-label={tooltip}
          >
            ?
            <span className="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 hidden group-hover:block w-56 p-2 text-xs text-gray-200 bg-gray-800 border border-gray-700 rounded-lg shadow-lg z-10">
              {tooltip}
            </span>
          </span>
        )}
      </div>
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        aria-label={label}
        onClick={() => onChange(!checked)}
        className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
          checked ? "bg-teal-600" : "bg-gray-700"
        }`}
      >
        <span
          className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
            checked ? "translate-x-6" : "translate-x-1"
          }`}
        />
      </button>
    </div>
  );
}

export default function GamePreferencesSettings() {
  const { preferences, setPreference } = useGamePreferences();

  return (
    <div className="p-6 bg-gray-900 rounded-xl shadow-lg border border-gray-800 text-white max-w-md mx-auto mt-10">
      <h2 className="text-2xl font-bold mb-6 text-teal-400">
        Game Preferences
      </h2>

      <RadioGroup<PieceInputMethod>
        label="Piece Input Method"
        value={preferences.pieceInputMethod}
        onChange={(v) => setPreference("pieceInputMethod", v)}
        tooltip="Choose how you move pieces on the board."
        options={[
          { id: "drag", label: "Drag & Drop", description: "Drag pieces to move" },
          { id: "click", label: "Click-Click", description: "Click source then target" },
          { id: "both", label: "Both", description: "Use either method" },
        ]}
      />

      <RadioGroup<AutoQueenMode>
        label="Auto-Queen on Promotion"
        value={preferences.autoQueen}
        onChange={(v) => setPreference("autoQueen", v)}
        tooltip="Controls how pawn promotion is handled."
        options={[
          { id: "always", label: "Always Queen", description: "Auto-promote to Queen" },
          { id: "prompt", label: "Always Prompt Choice", description: "Ask which piece to promote to" },
          { id: "premoves", label: "Auto-Queen in Pre-moves only", description: "Auto-queen only for pre-moves" },
        ]}
      />

      <RadioGroup<LegalMoveDots>
        label="Show Legal Move Dots"
        value={preferences.showLegalMoveDots}
        onChange={(v) => setPreference("showLegalMoveDots", v)}
        tooltip="Display dots on squares where a selected piece can move."
        options={[
          { id: "enabled", label: "Enabled", description: "Show legal move indicators" },
          { id: "disabled", label: "Disabled", description: "Hide legal move indicators" },
        ]}
      />

      <div className="border-t border-gray-800 pt-4 mb-4">
        <Toggle
          label="Require Checkmark on Correspondence games"
          checked={preferences.confirmMoveCorrespondence}
          onChange={(v) => setPreference("confirmMoveCorrespondence", v)}
          tooltip="Require explicit confirmation before sending a move in correspondence games."
        />
      </div>

      <RadioGroup<BoardCoordinates>
        label="Board Coordinates"
        value={preferences.boardCoordinates}
        onChange={(v) => setPreference("boardCoordinates", v)}
        tooltip="Choose where to display board coordinate labels."
        options={[
          { id: "inside", label: "Inside Board", description: "Show coordinates on the squares" },
          { id: "outside", label: "Outside Board", description: "Show coordinates around the board" },
          { id: "hidden", label: "Hidden", description: "Do not show coordinates" },
        ]}
      />

      <p className="text-sm text-gray-400 mt-4">
        Changes are saved automatically and apply to all your games.
      </p>
    </div>
  );
}
