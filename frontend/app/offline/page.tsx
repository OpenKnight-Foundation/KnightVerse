"use client";

import React from "react";
import { useRouter } from "next/navigation";
import OfflinePlayView from "@/components/chess/OfflinePlayView";

/**
 * FE-122 (#1363): route that mounts `OfflinePlayView`. The component only
 * renders its affordance while the browser is offline, so an explanatory note
 * is shown above it for online visitors.
 *
 * Starting an offline game against Stockfish (WASM worker + ServiceWorker
 * caching) is a separate backend/e2e dependency.
 */
export default function OfflinePlayPage() {
  const router = useRouter();

  return (
    <div
      className="min-h-screen p-4 md:p-8 text-white"
      role="main"
      aria-label="Offline play"
    >
      <div className="max-w-3xl mx-auto">
        <h1 className="text-3xl font-extrabold bg-gradient-to-r from-teal-400 to-blue-500 bg-clip-text text-transparent">
          Play Offline
        </h1>
        <p className="text-gray-400 text-sm mt-1 mb-6">
          When your connection drops you can keep playing against Stockfish.
        </p>
        <OfflinePlayView
          onStartOfflineGame={(elo) => router.push(`/play/practice?elo=${elo}`)}
        />
      </div>
    </div>
  );
}
