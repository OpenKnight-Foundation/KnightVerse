"use client";

import React from "react";

import { useAppContext } from "@/context/walletContext";
import { describeNetwork } from "@/lib/walletNetwork";
import { NETWORK_PASSPHRASE } from "@/lib/api";

/**
 * Warns when Freighter is on a different Stellar network than the app (FE-164).
 *
 * Signing is also blocked in the wallet context, so this banner explains why
 * and offers a re-check for after the user switches networks in Freighter.
 */
export function NetworkMismatchBanner() {
  const { status, walletNetwork, networkMismatch, refreshWalletNetwork } =
    useAppContext();
  const [isChecking, setIsChecking] = React.useState(false);

  if (status !== "connected" || !networkMismatch) return null;

  const handleRecheck = async () => {
    setIsChecking(true);
    try {
      await refreshWalletNetwork();
    } finally {
      setIsChecking(false);
    }
  };

  return (
    <div
      role="alert"
      aria-live="assertive"
      className="mb-4 flex flex-wrap items-center gap-3 rounded-xl border border-amber-500/40 bg-amber-500/10 px-4 py-3 text-sm text-amber-200"
    >
      <span aria-hidden="true">⚠</span>
      <p className="flex-1">
        Freighter is on {describeNetwork(walletNetwork)}, but KnightVerse runs on{" "}
        {describeNetwork(NETWORK_PASSPHRASE)}. Switch networks in Freighter to
        play staked games or move funds.
      </p>
      <button
        type="button"
        onClick={() => void handleRecheck()}
        disabled={isChecking}
        className="rounded-lg border border-amber-400/60 px-3 py-1 font-semibold text-amber-100 hover:bg-amber-400/20 disabled:opacity-50"
      >
        {isChecking ? "Checking…" : "Re-check network"}
      </button>
    </div>
  );
}
