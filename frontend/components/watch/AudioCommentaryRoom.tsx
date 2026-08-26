"use client";

import { useState } from "react";

interface AudioCommentaryRoomProps {
  commentatorName: string;
  canBroadcast?: boolean;
}

export function AudioCommentaryRoom({
  commentatorName,
  canBroadcast = false,
}: AudioCommentaryRoomProps) {
  const [joined, setJoined] = useState(false);
  const [muted, setMuted] = useState(true);
  const [volume, setVolume] = useState(80);
  const [speaking, setSpeaking] = useState(false);

  // Real signaling/peer-connection setup lives in useWebRTCAudio (see hook below).
  const handleJoin = () => {
    setJoined(true);
    setSpeaking(canBroadcast);
  };

  return (
    <div className="rounded-lg border p-3">
      <div className="flex items-center gap-2">
        <div
          className={`h-8 w-8 rounded-full border-2 ${
            speaking ? "border-green-500 animate-pulse" : "border-muted"
          }`}
        />
        <span className="text-sm font-medium">{commentatorName}</span>
      </div>

      {!joined ? (
        <button
          onClick={handleJoin}
          className="mt-2 rounded-md bg-primary px-3 py-1.5 text-sm text-primary-foreground"
        >
          Join Audio Stream
        </button>
      ) : (
        <div className="mt-2 flex items-center gap-3">
          <button onClick={() => setMuted((m) => !m)} className="rounded-md border px-2 py-1 text-xs">
            {muted ? "Unmute" : "Mute"}
          </button>
          <input
            type="range"
            min={0}
            max={100}
            value={volume}
            onChange={(e) => setVolume(Number(e.target.value))}
            aria-label="Volume"
          />
        </div>
      )}
    </div>
  );
}
