"use client";

import React from "react";

import type { ClaimStatus } from "@/hook/useClaimWinnings";

interface ClaimWinningsPanelProps {
  status: ClaimStatus;
  hash: string | null;
  error: string | null;
  onClaim: () => void;
}

const PROGRESS_LABEL: Partial<Record<ClaimStatus, string>> = {
  fetching: "Fetching the signed result…",
  signing: "Waiting for Freighter to sign…",
  submitting: "Submitting the claim to Stellar…",
};

/**
 * Claim control shown in the post-game overlay after a won staked game
 * (FE-163). Shows progress, the confirmed transaction hash, and a retry on
 * failure.
 */
export function ClaimWinningsPanel({
  status,
  hash,
  error,
  onClaim,
}: ClaimWinningsPanelProps) {
  const isPending =
    status === "fetching" || status === "signing" || status === "submitting";

  if (status === "confirmed") {
    return (
      <div
        role="status"
        aria-live="polite"
        className="w-full rounded-xl border border-emerald-500/40 bg-emerald-500/10 px-4 py-3 text-left text-sm text-emerald-200"
      >
        <p className="font-semibold">Winnings claimed</p>
        {hash ? (
          <p className="mt-1 break-all font-mono text-xs text-emerald-100/90">
            Transaction: {hash}
          </p>
        ) : (
          <p className="mt-1 text-xs text-emerald-100/80">
            The claim was submitted to the network.
          </p>
        )}
      </div>
    );
  }

  return (
    <div className="w-full space-y-2">
      <button
        type="button"
        onClick={onClaim}
        disabled={isPending}
        aria-busy={isPending}
        className="w-full flex items-center justify-center gap-2 px-5 py-3 bg-gradient-to-r from-amber-500 to-yellow-600 hover:from-amber-400 hover:to-yellow-500 text-black font-semibold rounded-xl shadow-lg shadow-amber-500/20 transition-all duration-300 hover:scale-[1.02] active:scale-[0.98] disabled:opacity-60 disabled:hover:scale-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-400"
      >
        {isPending
          ? PROGRESS_LABEL[status]
          : status === "error"
            ? "Retry claiming winnings"
            : "Claim winnings"}
      </button>
      {status === "error" && error && (
        <p role="alert" className="text-xs text-red-300">
          {error}
        </p>
      )}
    </div>
  );
}

export default ClaimWinningsPanel;
