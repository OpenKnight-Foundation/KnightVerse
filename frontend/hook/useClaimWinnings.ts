"use client";

import { useCallback, useEffect, useRef, useState } from "react";

import { useAuth } from "@/context/authContext";
import { useAppContext } from "@/context/walletContext";
import { authFetch, endpoints } from "@/lib/api";
import {
  claimGameWin,
  getGameContractId,
  parseWinClaim,
  type WinClaim,
} from "@/contracts/client";

/** Progress of an on-chain winnings claim (FE-163). */
export type ClaimStatus =
  | "idle"
  | "fetching"
  | "signing"
  | "submitting"
  | "confirmed"
  | "error";

/** The wager the matchmaking match was queued with. */
export interface StakeInfo {
  token: string;
  amount: number;
}

interface MatchDetails {
  stake_info?: { token?: string; amount?: number } | null;
}

interface UseClaimWinningsOptions {
  /**
   * Id of the finished game. Matchmaking uses the match id as the game id, so
   * the match record tells us whether a wager is at stake.
   */
  gameId: string | null | undefined;
  /** True only when the local player won. */
  enabled: boolean;
}

interface UseClaimWinningsReturn {
  status: ClaimStatus;
  /** Ledger transaction hash once the claim is confirmed. */
  hash: string | null;
  error: string | null;
  /** The signed claim, once the backend has issued it. */
  claim: WinClaim | null;
  /** The wager being claimed, or `null` for an unstaked game. */
  stake: StakeInfo | null;
  isPending: boolean;
  /**
   * True when claiming is not possible or not applicable (lost game, unstaked
   * match, no wallet, no contract configured, or the match lookup failed), so
   * the claim control stays hidden.
   */
  isUnavailable: boolean;
  claimWinnings: () => Promise<void>;
  reset: () => void;
}

/**
 * Claims a staked game's winnings on-chain (FE-163).
 *
 * The backend signs the result (`GET /v1/games/{id}/win-claim`, BE-151) and the
 * player's wallet submits `game_contract.claim_win` with that signature. The
 * hook stays inert unless the local player won a staked game and the game
 * contract is configured, so lost and unstaked games never show a claim
 * control.
 */
export function useClaimWinnings({
  gameId,
  enabled,
}: UseClaimWinningsOptions): UseClaimWinningsReturn {
  const { address, signTransaction } = useAppContext();
  const { accessToken, refreshToken, logout } = useAuth();

  const [status, setStatus] = useState<ClaimStatus>("idle");
  const [hash, setHash] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [claim, setClaim] = useState<WinClaim | null>(null);
  const [stake, setStake] = useState<StakeInfo | null>(null);

  // Guards against a claim landing after unmount or after the game changed.
  const gameIdRef = useRef(gameId);
  gameIdRef.current = gameId;

  const reset = useCallback(() => {
    setStatus("idle");
    setHash(null);
    setError(null);
    setClaim(null);
  }, []);

  useEffect(() => {
    reset();
    setStake(null);
  }, [gameId, reset]);

  // Only a won staked game has anything to claim, so look up the wager first.
  useEffect(() => {
    if (!enabled || !gameId) return;
    let cancelled = false;

    const loadStake = async () => {
      try {
        const res = await fetch(endpoints.matchmaking.match(gameId));
        if (!res.ok) return;
        const match: MatchDetails = await res.json();
        const info = match.stake_info;
        if (cancelled || !info?.token || !info.amount) return;
        setStake({ token: info.token, amount: info.amount });
      } catch {
        // No match record (or the backend is unreachable): stay hidden.
      }
    };

    void loadStake();
    return () => {
      cancelled = true;
    };
  }, [enabled, gameId]);

  const isUnavailable =
    !enabled || !gameId || !address || !stake || !getGameContractId();

  const claimWinnings = useCallback(async () => {
    if (!gameId || !address) {
      setError("Connect your wallet to claim your winnings.");
      setStatus("error");
      return;
    }
    if (!getGameContractId()) {
      setError("The game contract is not configured for this deployment.");
      setStatus("error");
      return;
    }

    const requestGameId = gameId;
    setError(null);
    setHash(null);
    setStatus("fetching");

    try {
      let signed = claim;
      if (!signed) {
        const res = await authFetch(
          endpoints.games.winClaim(requestGameId),
          {},
          { getToken: () => accessToken, refresh: refreshToken, logout },
        );
        if (!res.ok) {
          throw new Error(
            res.status === 404 || res.status === 501
              ? "Signed results are not available yet. Please try again later."
              : `Could not fetch the signed result (HTTP ${res.status}).`,
          );
        }
        signed = parseWinClaim(await res.json());
        if (!signed) throw new Error("The signed result was malformed.");
        if (signed.winner !== address) {
          throw new Error("The signed result names a different winner.");
        }
        if (gameIdRef.current === requestGameId) setClaim(signed);
      }

      if (gameIdRef.current !== requestGameId) return;
      setStatus("signing");

      const result = await claimGameWin({
        claim: signed,
        signer: {
          address,
          sign: async (xdr: string) => {
            const signedXdr = await signTransaction(xdr);
            // The envelope is signed; from here we only wait on the ledger.
            if (gameIdRef.current === requestGameId) setStatus("submitting");
            return signedXdr;
          },
        },
      });

      if (gameIdRef.current !== requestGameId) return;
      setHash(result.hash ?? null);
      setStatus("confirmed");
    } catch (err) {
      if (gameIdRef.current !== requestGameId) return;
      setError(
        err instanceof Error ? err.message : "Claiming the winnings failed.",
      );
      setStatus("error");
    }
  }, [address, accessToken, claim, gameId, logout, refreshToken, signTransaction]);

  return {
    status,
    hash,
    error,
    claim,
    stake,
    isPending:
      status === "fetching" || status === "signing" || status === "submitting",
    isUnavailable,
    claimWinnings,
    reset,
  };
}
