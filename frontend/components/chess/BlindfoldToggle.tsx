"use client";

import React from "react";

interface BlindfoldToggleProps {
  blindfoldMode: boolean;
  onToggle: (enabled: boolean) => void;
}

/**
 * FE-51: blindfold mode toggle. Consumers apply `blindfoldMode` to piece
 * rendering (e.g. `opacity: 0` on piece SVGs) while keeping hitboxes and
 * legal-move logic untouched — this component only owns the on/off state.
 */
const BlindfoldToggle: React.FC<BlindfoldToggleProps> = ({
  blindfoldMode,
  onToggle,
}) => {
  return (
    <label className="blindfold-toggle flex items-center gap-2 text-sm">
      <input
        type="checkbox"
        checked={blindfoldMode}
        onChange={(e) => onToggle(e.target.checked)}
        aria-label="Blindfold mode"
      />
      Blindfold mode (hide pieces)
    </label>
  );
};

export const pieceStyleForBlindfold = (
  blindfoldMode: boolean
): React.CSSProperties => ({
  opacity: blindfoldMode ? 0 : 1,
  pointerEvents: "auto",
});

export default BlindfoldToggle;
