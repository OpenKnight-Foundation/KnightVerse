"use client";

import React, { useEffect, useState } from "react";

interface OfflinePlayViewProps {
  onStartOfflineGame: (elo: number) => void;
}

/**
 * FE-49: minimal offline play-vs-Stockfish entry point.
 * Actual Stockfish WASM worker + ServiceWorker caching are separate
 * follow-up work — this wires the "Play Offline" affordance and Elo pick.
 */
const OfflinePlayView: React.FC<OfflinePlayViewProps> = ({
  onStartOfflineGame,
}) => {
  const [isOffline, setIsOffline] = useState(false);
  const [elo, setElo] = useState(1200);

  useEffect(() => {
    setIsOffline(!navigator.onLine);
    const goOffline = () => setIsOffline(true);
    const goOnline = () => setIsOffline(false);
    window.addEventListener("offline", goOffline);
    window.addEventListener("online", goOnline);
    return () => {
      window.removeEventListener("offline", goOffline);
      window.removeEventListener("online", goOnline);
    };
  }, []);

  if (!isOffline) return null;

  return (
    <div className="offline-play-view p-4 rounded-lg border border-yellow-500/40 bg-yellow-500/10">
      <p className="text-sm mb-2">You&apos;re offline. Practice vs Stockfish instead?</p>
      <input
        type="range"
        min={800}
        max={2500}
        step={100}
        value={elo}
        onChange={(e) => setElo(Number(e.target.value))}
        aria-label="Stockfish Elo"
      />
      <span className="ml-2 text-sm">{elo} Elo</span>
      <button
        onClick={() => onStartOfflineGame(elo)}
        className="block mt-3 px-4 py-2 rounded bg-yellow-600 text-white"
      >
        Play Offline
      </button>
    </div>
  );
};

export default OfflinePlayView;
