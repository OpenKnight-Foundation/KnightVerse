"use client";

import React from "react";

interface BerserkButtonProps {
  isArenaMatch: boolean;
  moveNumber: number;
  onBerserk: () => void;
}

/**
 * FE-52: Berserk button for Arena tournaments. Only active before the
 * player's first move; dispatches a `BerserkMove` event upstream via
 * `onBerserk`, which the game socket layer is responsible for sending.
 */
const BerserkButton: React.FC<BerserkButtonProps> = ({
  isArenaMatch,
  moveNumber,
  onBerserk,
}) => {
  if (!isArenaMatch || moveNumber > 0) return null;

  return (
    <button
      onClick={onBerserk}
      aria-label="Berserk — halve your clock for bonus tournament points"
      className="berserk-button px-4 py-2 rounded-md font-bold text-white bg-red-700 border-2 border-orange-400 animate-pulse shadow-[0_0_12px_rgba(255,80,0,0.8)]"
    >
      🔥 BERSERK
    </button>
  );
};

export default BerserkButton;
