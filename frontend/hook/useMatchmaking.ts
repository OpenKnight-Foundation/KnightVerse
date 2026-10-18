import { useCallback, useEffect, useRef, useState } from "react";

import type { ChessVariant } from "@/lib/chessVariants";

import { endpoints } from "@/lib/api";
import { useAuth } from "@/context/authContext";
import { useAppContext } from "@/context/walletContext";

export type MatchmakingStatus =
  | "idle"
  | "searching"
  | "match_found"
  | "error";

/** How often to ask the backend whether our queued request has been paired. */
export const MATCHMAKING_POLL_INTERVAL_MS = 2000;

/** Rating sent until the backend derives it from the player record (BE-111). */
const DEFAULT_ELO = 1200;

interface MatchmakingResponse {
  status: string;
  match_id: string | null;
  request_id: string;
}

interface StatusResponse {
  status: string;
  match_id?: string;
}

interface MatchDetails {
  id: string;
  player1: { wallet_address: string };
  player2: { wallet_address: string };
}

interface UseMatchmakingReturn {
  status: MatchmakingStatus;
  gameId: string | null;
  playerColor: "white" | "black" | null;
  error: string | null;
  joinMatchmaking: (
    matchType?: "Rated" | "Casual",
    timeControl?: ChessVariant,
  ) => Promise<void>;
  cancelMatchmaking: () => void;
}

/**
 * Joins the backend matchmaking queue for the selected mode and polls
 * `/v1/matchmaking/status/{request_id}` until the request is paired (#FE-98).
 *
 * The player is identified by their linked wallet (#FE-97), falling back to
 * the connected Freighter address.
 */
export function useMatchmaking(): UseMatchmakingReturn {
  const { user } = useAuth();
  const { address: connectedAddress } = useAppContext();
  const walletAddress = user?.wallet_address ?? connectedAddress ?? null;

  const [status, setStatus] = useState<MatchmakingStatus>("idle");
  const [gameId, setGameId] = useState<string | null>(null);
  const [playerColor, setPlayerColor] = useState<"white" | "black" | null>(null);
  const [error, setError] = useState<string | null>(null);

  const requestIdRef = useRef<string | null>(null);
  const pollTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Bumped on every join/cancel so stale polls from an earlier search stop.
  const searchRef = useRef(0);

  const stopPolling = useCallback(() => {
    if (pollTimerRef.current) {
      clearTimeout(pollTimerRef.current);
      pollTimerRef.current = null;
    }
  }, []);

  const fail = useCallback(
    (message: string) => {
      stopPolling();
      requestIdRef.current = null;
      setError(message);
      setStatus("error");
    },
    [stopPolling],
  );

  const enterMatch = useCallback(
    async (matchId: string, search: number, myAddress: string) => {
      let color: "white" | "black" = "white";
      try {
        const res = await fetch(endpoints.matchmaking.match(matchId));
        if (res.ok) {
          const details: MatchDetails = await res.json();
          // player1 is the player who was already waiting; they take White.
          color = details.player1.wallet_address === myAddress ? "white" : "black";
        }
      } catch {
        // Fall through with White; the game server's state is authoritative.
      }
      if (searchRef.current !== search) return;
      stopPolling();
      requestIdRef.current = null;
      setGameId(matchId);
      setPlayerColor(color);
      setStatus("match_found");
    },
    [stopPolling],
  );

  const pollStatus = useCallback(
    (requestId: string, search: number, myAddress: string) => {
      pollTimerRef.current = setTimeout(async () => {
        if (searchRef.current !== search) return;
        try {
          const res = await fetch(endpoints.matchmaking.status(requestId));
          if (searchRef.current !== search) return;
          if (res.status === 404) {
            fail("Matchmaking request expired. Please search again.");
            return;
          }
          if (!res.ok) throw new Error(`Matchmaking status failed: ${res.status}`);
          const body: StatusResponse = await res.json();
          if (body.match_id) {
            await enterMatch(body.match_id, search, myAddress);
          } else {
            pollStatus(requestId, search, myAddress);
          }
        } catch {
          // Transient network error: keep polling rather than dropping the search.
          if (searchRef.current === search) pollStatus(requestId, search, myAddress);
        }
      }, MATCHMAKING_POLL_INTERVAL_MS);
    },
    [enterMatch, fail],
  );

  const joinMatchmaking = useCallback(
    async (matchType: "Rated" | "Casual" = "Casual", timeControl: ChessVariant = "standard") => {
      stopPolling();
      const search = ++searchRef.current;
      setError(null);
      setGameId(null);
      setPlayerColor(null);

      if (!walletAddress) {
        fail("Connect your wallet to play online.");
        return;
      }
      setStatus("searching");

      try {
        const url = matchType === "Rated" ? endpoints.matchmaking.joinRated() : endpoints.matchmaking.joinCasual();
        const res = await fetch(url, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            wallet_address: walletAddress,
            elo: DEFAULT_ELO,
            invite_address: null,
            ...(matchType === "Rated" ? { max_elo_diff: null } : {}),
            time_control: timeControl,
          }),
        });
        if (!res.ok) throw new Error(`Matchmaking failed: ${res.status}`);

        const json: MatchmakingResponse = await res.json();
        if (searchRef.current !== search) return;
        requestIdRef.current = json.request_id;

        if (json.match_id) {
          await enterMatch(json.match_id, search, walletAddress);
        } else {
          pollStatus(json.request_id, search, walletAddress);
        }
      } catch (err) {
        if (searchRef.current === search) fail(err instanceof Error ? err.message : "Unknown error");
      }
    },
    [walletAddress, enterMatch, pollStatus, fail, stopPolling],
  );

  const cancelMatchmaking = useCallback(() => {
    searchRef.current++;
    stopPolling();
    if (requestIdRef.current) {
      fetch(endpoints.matchmaking.cancel(), {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ request_id: requestIdRef.current }),
      }).catch(() => {});
      requestIdRef.current = null;
    }
    setStatus("idle");
    setGameId(null);
    setPlayerColor(null);
    setError(null);
  }, [stopPolling]);

  useEffect(() => () => stopPolling(), [stopPolling]);

  return { status, gameId, playerColor, error, joinMatchmaking, cancelMatchmaking };
}
