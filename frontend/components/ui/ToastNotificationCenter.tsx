"use client";

import { useState, useCallback } from "react";

export type NotificationType =
  | "DirectChallenge"
  | "TournamentRoundStarting"
  | "FriendRequest"
  | "StakingPayoutConfirmed";

export interface GameNotification {
  id: string;
  type: NotificationType;
  message: string;
  challengerAvatarUrl?: string;
  challengerRating?: number;
  timeControl?: string;
  stake?: string;
  onAccept?: () => void;
  onDecline?: () => void;
}

const AUTO_DISMISS_MS = 15000;

/** Minimal stacking notification center for challenges, tournaments, and mentions. */
export function ToastNotificationCenter() {
  const [notifications, setNotifications] = useState<GameNotification[]>([]);

  const dismiss = useCallback((id: string) => {
    setNotifications((prev) => prev.filter((n) => n.id !== id));
    // eslint-disable-next-line @typescript-eslint/no-unused-vars
  }, []);

  return (
    <div className="fixed bottom-4 right-4 z-50 flex flex-col gap-2">
      {notifications.map((n) => (
        <div
          key={n.id}
          role="status"
          className="w-80 rounded-lg border bg-background p-3 shadow-lg"
        >
          <p className="text-sm font-medium">{n.message}</p>
          {n.type === "DirectChallenge" && (
            <div className="mt-2 flex gap-2">
              <button
                onClick={() => {
                  n.onAccept?.();
                  dismiss(n.id);
                }}
                className="rounded bg-primary px-2 py-1 text-xs text-primary-foreground"
              >
                Accept
              </button>
              <button
                onClick={() => {
                  n.onDecline?.();
                  dismiss(n.id);
                }}
                className="rounded border px-2 py-1 text-xs"
              >
                Decline
              </button>
            </div>
          )}
        </div>
      ))}
    </div>
  );
}
