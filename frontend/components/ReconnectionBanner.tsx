"use client";

import { useEffect, useState } from "react";

interface ReconnectionBannerProps {
  isDisconnected: boolean;
  onReconnected?: () => void;
  gracePeriodSeconds?: number;
}

export function ReconnectionBanner({
  isDisconnected,
  onReconnected,
  gracePeriodSeconds = 60,
}: ReconnectionBannerProps) {
  const [secondsLeft, setSecondsLeft] = useState(gracePeriodSeconds);
  const [reconnected, setReconnected] = useState(false);

  useEffect(() => {
    if (!isDisconnected) return;
    setSecondsLeft(gracePeriodSeconds);
    setReconnected(false);

    const interval = setInterval(() => {
      setSecondsLeft((s) => (s > 0 ? s - 1 : 0));
    }, 1000);

    return () => clearInterval(interval);
  }, [isDisconnected, gracePeriodSeconds]);

  useEffect(() => {
    if (!isDisconnected && secondsLeft !== gracePeriodSeconds) {
      setReconnected(true);
      onReconnected?.();
      const timeout = setTimeout(() => setReconnected(false), 3000);
      return () => clearTimeout(timeout);
    }
  }, [isDisconnected, secondsLeft, gracePeriodSeconds, onReconnected]);

  if (!isDisconnected && !reconnected) return null;

  return (
    <div
      role="status"
      className={`fixed top-4 left-1/2 -translate-x-1/2 z-50 rounded-lg px-4 py-2 shadow-lg text-sm font-medium ${
        reconnected ? "bg-green-600 text-white" : "bg-amber-500 text-black"
      }`}
    >
      {reconnected
        ? "Reconnected"
        : `Reconnecting... ${secondsLeft}s left before timeout`}
    </div>
  );
}
