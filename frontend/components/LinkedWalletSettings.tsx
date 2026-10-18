"use client";

import React from "react";

import { useAuth } from "@/context/authContext";
import { useWalletLink } from "@/hook/useWalletLink";

function shorten(address: string) {
  return `${address.slice(0, 6)}…${address.slice(-6)}`;
}

/** Shows and manages the Stellar wallet linked to the signed-in account (#FE-97). */
export default function LinkedWalletSettings() {
  const { isAuthenticated } = useAuth();
  const { linkedWallet, connectedAddress, isMismatched, isLinking, error, linkWallet, unlinkWallet } =
    useWalletLink();

  return (
    <section
      aria-labelledby="linked-wallet-title"
      className="rounded-2xl border border-gray-700/50 bg-gray-900/60 p-6 text-white"
    >
      <h2 id="linked-wallet-title" className="text-lg font-bold">Linked wallet</h2>
      <p className="mt-1 text-sm text-gray-400">
        The Stellar account used for staking and matchmaking.
      </p>

      {!isAuthenticated ? (
        <p className="mt-4 text-sm text-gray-400">Sign in to link a wallet to your account.</p>
      ) : (
        <div className="mt-4 space-y-3 text-sm">
          <p>
            {linkedWallet ? (
              <>
                Linked: <span className="font-mono" title={linkedWallet}>{shorten(linkedWallet)}</span>
              </>
            ) : (
              "No wallet linked yet."
            )}
          </p>
          {isMismatched && (
            <p role="status" className="text-amber-300">
              Your connected wallet ({shorten(connectedAddress!)}) differs from the linked one.
            </p>
          )}
          <div className="flex flex-wrap gap-2">
            {connectedAddress && connectedAddress !== linkedWallet && (
              <button
                onClick={() => void linkWallet()}
                disabled={isLinking}
                className="rounded-xl bg-teal-500 px-4 py-2 font-bold text-black disabled:opacity-50"
              >
                {isLinking ? "Waiting for Freighter…" : `Link ${shorten(connectedAddress)}`}
              </button>
            )}
            {!connectedAddress && <span className="text-gray-400">Connect Freighter to link a wallet.</span>}
            {linkedWallet && (
              <button
                onClick={() => void unlinkWallet()}
                disabled={isLinking}
                className="rounded-xl border border-gray-600 px-4 py-2 disabled:opacity-50"
              >
                Unlink
              </button>
            )}
          </div>
          {error && <p role="alert" className="text-red-300">{error}</p>}
        </div>
      )}
    </section>
  );
}
