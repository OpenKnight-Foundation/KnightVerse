"use client";

import React from "react";
import { AudioCommentaryRoom } from "@/components/watch/AudioCommentaryRoom";

/**
 * FE-122 (#1363): route that mounts the audio commentary room so spectators can
 * join a live broadcast. Real WebRTC signalling is a separate backend
 * dependency; this wires the entry point and room UI.
 */
export default function WatchCommentaryPage() {
  return (
    <div
      className="min-h-screen p-4 md:p-8 text-white"
      role="main"
      aria-label="Audio commentary"
    >
      <div className="max-w-3xl mx-auto">
        <h1 className="text-3xl font-extrabold bg-gradient-to-r from-teal-400 to-blue-500 bg-clip-text text-transparent">
          Audio Commentary
        </h1>
        <p className="text-gray-400 text-sm mt-1 mb-6">
          Listen to live commentary while you watch the game.
        </p>
        <AudioCommentaryRoom commentatorName="Guest Commentator" canBroadcast={false} />
      </div>
    </div>
  );
}
