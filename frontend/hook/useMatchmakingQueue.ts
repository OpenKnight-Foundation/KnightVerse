"use client";

/**
 * useMatchmakingQueue (#555)
 *
 * React hook that manages the full matchmaking lifecycle:
 *   - Joining / leaving the Redis-backed queue
 *   - Polling for a match every `pollIntervalMs` milliseconds
 *   - Exposing queue status and match result to the UI
 */

import { useCallback, useEffect, useRef, useState } from "react";
import {
  type MatchRequest,
  type MatchmakingResponse,
  type QueueStatus,
  getQueueStatus,
  joinQueue,
  leaveQueue,
} from "@/lib/matchmakingService";

export type QueueState = "idle" | "searching" | "matched" | "error";

export interface UseMatchmakingQueueResult {
  state: QueueState;
  requestId: string | null;
  matchId: string | null;
  queueStatus: QueueStatus | null;
  error: string | null;
  join: (request: MatchRequest) => Promise<void>;
  leave: () => Promise<void>;
}

const DEFAULT_POLL_INTERVAL_MS = 3_000;

export function useMatchmakingQueue(
  pollIntervalMs = DEFAULT_POLL_INTERVAL_MS,
): UseMatchmakingQueueResult {
  const [state, setState] = useState<QueueState>("idle");
  const [requestId, setRequestId] = useState<string | null>(null);
  const [matchId, setMatchId] = useState<string | null>(null);
  const [queueStatus, setQueueStatus] = useState<QueueStatus | null>(null);
  const [error, setError] = useState<string | null>(null);

  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const requestIdRef = useRef<string | null>(null);

  const stopPolling = useCallback(() => {
    if (pollRef.current !== null) {
      clearInterval(pollRef.current);
      pollRef.current = null;
    }
  }, []);

  const startPolling = useCallback(() => {
    stopPolling();

    pollRef.current = setInterval(async () => {
      const id = requestIdRef.current;
      if (!id) return;

      try {
        const status = await getQueueStatus(id);

        if (status === null) {
          // Request no longer in queue — assume matched or expired
          stopPolling();
          setState("matched");
          return;
        }

        setQueueStatus(status);
      } catch (err) {
        stopPolling();
        setError(err instanceof Error ? err.message : "Polling error");
        setState("error");
      }
    }, pollIntervalMs);
  }, [pollIntervalMs, stopPolling]);

  const join = useCallback(
    async (request: MatchRequest) => {
      setError(null);
      setState("searching");

      try {
        const response: MatchmakingResponse = await joinQueue(request);

        if (response.matchId) {
          // Instant match found
          setMatchId(response.matchId);
          setState("matched");
          return;
        }

        setRequestId(response.requestId);
        requestIdRef.current = response.requestId;
        startPolling();
      } catch (err) {
        setError(err instanceof Error ? err.message : "Failed to join queue");
        setState("error");
      }
    },
    [startPolling],
  );

  const leave = useCallback(async () => {
    stopPolling();

    const id = requestIdRef.current;
    if (id) {
      try {
        await leaveQueue(id);
      } catch {
        // Best-effort; reset state regardless
      }
    }

    requestIdRef.current = null;
    setRequestId(null);
    setMatchId(null);
    setQueueStatus(null);
    setError(null);
    setState("idle");
  }, [stopPolling]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      stopPolling();
    };
  }, [stopPolling]);

  return { state, requestId, matchId, queueStatus, error, join, leave };
}
