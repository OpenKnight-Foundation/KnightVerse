"use client";

import React, { useState, useCallback } from "react";
import * as Dialog from "@radix-ui/react-dialog";
import { useAppContext } from "@/context/walletContext";
import { useToast } from "@/components/ui/toast";
import { useTrackedTransaction } from "@/hook/useTrackedTransaction";

interface WalletConnectModalProps {
  isOpen: boolean;
  onClose: () => void;
}

type TxStep = "idle" | "pending" | "success" | "error";

export function WalletConnectModal({
  isOpen,
  onClose,
}: WalletConnectModalProps) {
  const {
    connectWallet,
    disconnectWallet,
    address,
    sendXLM,
  } = useAppContext();
  const { addToast } = useToast();
  const [isConnecting, setIsConnecting] = useState(false);
  const [txStep, setTxStep] = useState<TxStep>("idle");
  const [showXlmSend, setShowXlmSend] = useState(false);
  const [xlmDestination, setXlmDestination] = useState("");
  const [xlmAmount, setXlmAmount] = useState("");

  const { execute: executeTrackedTx } = useTrackedTransaction({
    type: "payment",
    label: `Send ${xlmAmount || "?"} XLM`,
    amount: xlmAmount,
    destination: xlmDestination,
  });

  const handleConnect = useCallback(async () => {
    setIsConnecting(true);
    try {
      await connectWallet();
      addToast({
        severity: "success",
        title: "Wallet Connected",
        detail: "Your Freighter wallet is now connected to XLMate.",
      });
    } catch (err) {
      addToast({
        severity: "error",
        title: "Connection Failed",
        detail:
          err instanceof Error ? err.message : "Unable to connect wallet.",
      });
    } finally {
      setIsConnecting(false);
    }
  }, [connectWallet, addToast]);

  const handleDisconnect = useCallback(async () => {
    try {
      await disconnectWallet();
      setTxStep("idle");
      setShowXlmSend(false);
      addToast({
        severity: "info",
        title: "Wallet Disconnected",
        detail: "Your wallet has been disconnected.",
      });
      onClose();
    } catch {
      addToast({
        severity: "error",
        title: "Disconnect Failed",
        detail: "Could not disconnect wallet.",
      });
    }
  }, [disconnectWallet, addToast, onClose]);

  const handleSendXLM = useCallback(async () => {
    if (!address || !xlmDestination || !xlmAmount) return;
    setTxStep("pending");

    const result = await executeTrackedTx(async () => {
      return await sendXLM(xlmDestination, xlmAmount);
    });

    if (result !== undefined) {
      setTxStep("success");
      setTimeout(() => {
        setShowXlmSend(false);
        setXlmDestination("");
        setXlmAmount("");
        setTxStep("idle");
      }, 2000);
    } else {
      setTxStep("error");
      setTimeout(() => setTxStep("idle"), 3000);
    }
  }, [address, xlmDestination, xlmAmount, sendXLM, executeTrackedTx]);

  const truncateAddress = (addr: string) =>
    `${addr.slice(0, 6)}...${addr.slice(-4)}`;

  return (
    <Dialog.Root open={isOpen} onOpenChange={(open: boolean) => !open && onClose()}>
      <Dialog.Portal>
        {/* Overlay */}
        <Dialog.Overlay className="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm animate-overlay-in" />

        {/* Content */}
        <Dialog.Content className="fixed left-1/2 top-1/2 z-50 w-full max-w-md -translate-x-1/2 -translate-y-1/2 rounded-2xl border border-gray-600/50 bg-gray-900/80 backdrop-blur-2xl p-0 shadow-[0_0_50px_-12px_rgba(20,184,166,0.3)] animate-modal-in focus:outline-none">
          {/* Glowing Aura Background */}
          <div className="absolute -inset-1 rounded-2xl bg-gradient-to-r from-teal-500/20 to-blue-600/20 blur-xl z-[-1] opacity-70 pointer-events-none"></div>

          {/* Header gradient bar */}
          <div className="h-1.5 w-full rounded-t-2xl bg-gradient-to-r from-teal-400 via-blue-500 to-indigo-500 shadow-[0_2px_10px_rgba(45,212,191,0.5)]" />

          <div className="p-6 space-y-5">
            {/* Close button */}
            <Dialog.Close className="absolute top-5 right-5 text-gray-500 hover:text-white transition-colors">
              <svg
                xmlns="http://www.w3.org/2000/svg"
                className="h-5 w-5"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
                strokeWidth={2}
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  d="M6 18L18 6M6 6l12 12"
                />
              </svg>
            </Dialog.Close>

            {/* Title */}
            <Dialog.Title className="text-xl font-bold text-white">
              {address ? "Wallet Connected" : "Connect Wallet"}
            </Dialog.Title>
            <Dialog.Description className="text-sm text-gray-400">
              {address
                ? "Manage your Stellar wallet and send transactions."
                : "Connect your Freighter wallet to play, earn rewards, and transact on Stellar."}
            </Dialog.Description>

            {/* Connected state */}
            {address ? (
              <div className="space-y-4">
                {/* Address display */}
                <div className="flex items-center gap-3 p-3 rounded-xl bg-gray-800/60 border border-gray-700/50">
                  <div className="w-10 h-10 rounded-full bg-gradient-to-br from-teal-400 to-blue-600 flex items-center justify-center text-white text-xs font-bold">
                    {address.slice(0, 2).toUpperCase()}
                  </div>
                  <div className="flex-1 min-w-0">
                    <p className="text-sm font-mono text-gray-200 truncate">
                      {truncateAddress(address)}
                    </p>
                    <div className="flex items-center gap-1.5 mt-0.5">
                      <span className="w-2 h-2 rounded-full bg-emerald-400 animate-pulse-glow" />
                      <span className="text-xs text-emerald-400">
                        Connected
                      </span>
                    </div>
                  </div>
                  <button
                    onClick={() => {
                      navigator.clipboard.writeText(address);
                      addToast({
                        severity: "info",
                        title: "Address Copied",
                        detail: "Wallet address copied to clipboard.",
                      });
                    }}
                    className="p-2 rounded-lg hover:bg-gray-700/50 text-gray-400 hover:text-white transition-colors"
                    aria-label="Copy address"
                  >
                    <svg
                      xmlns="http://www.w3.org/2000/svg"
                      className="h-4 w-4"
                      fill="none"
                      viewBox="0 0 24 24"
                      stroke="currentColor"
                      strokeWidth={2}
                    >
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"
                      />
                    </svg>
                  </button>
                </div>

                {/* Action buttons */}
                <div className="space-y-2">
                  {!showXlmSend ? (
                    <button
                      onClick={() => setShowXlmSend(true)}
                      className="w-full py-2.5 rounded-xl bg-gradient-to-r from-teal-500/20 to-blue-600/20 border border-teal-500/30 hover:from-teal-500/30 hover:to-blue-600/30 text-teal-400 font-medium text-sm transition-all duration-300 hover:scale-[1.02] active:scale-[0.98]"
                    >
                      Send XLM
                    </button>
                  ) : (
                    <div className="space-y-3 p-3 rounded-xl bg-gray-800/40 border border-gray-700/30 animate-scale-in">
                      <input
                        type="text"
                        placeholder="Destination address"
                        aria-label="Destination address"
                        value={xlmDestination}
                        onChange={(e) => setXlmDestination(e.target.value)}
                        className="w-full px-3 py-2 rounded-lg bg-gray-800/60 border border-gray-700/50 text-gray-200 text-sm placeholder:text-gray-500 focus:outline-none focus:border-teal-500/50 transition-colors"
                      />
                      <input
                        type="number"
                        placeholder="Amount (XLM)"
                        aria-label="Amount in XLM"
                        value={xlmAmount}
                        onChange={(e) => setXlmAmount(e.target.value)}
                        step="0.001"
                        min="0"
                        className="w-full px-3 py-2 rounded-lg bg-gray-800/60 border border-gray-700/50 text-gray-200 text-sm placeholder:text-gray-500 focus:outline-none focus:border-teal-500/50 transition-colors"
                      />
                      <div className="flex gap-2">
                        <button
                          onClick={handleSendXLM}
                          disabled={
                            txStep === "pending" ||
                            !xlmDestination ||
                            !xlmAmount
                          }
                          className="flex-1 py-2 rounded-lg bg-gradient-to-r from-teal-500 to-blue-600 hover:from-teal-600 hover:to-blue-700 disabled:opacity-50 disabled:cursor-not-allowed text-white font-medium text-sm transition-all duration-300"
                        >
                          {txStep === "pending" ? (
                            <span className="flex items-center justify-center gap-2">
                              <svg
                                className="animate-spin h-4 w-4"
                                viewBox="0 0 24 24"
                              >
                                <circle
                                  className="opacity-25"
                                  cx="12"
                                  cy="12"
                                  r="10"
                                  stroke="currentColor"
                                  strokeWidth="4"
                                  fill="none"
                                />
                                <path
                                  className="opacity-75"
                                  fill="currentColor"
                                  d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
                                />
                              </svg>
                              Sending...
                            </span>
                          ) : txStep === "success" ? (
                            "Sent!"
                          ) : txStep === "error" ? (
                            "Retry"
                          ) : (
                            "Send"
                          )}
                        </button>
                        <button
                          onClick={() => {
                            setShowXlmSend(false);
                            setTxStep("idle");
                          }}
                          className="px-4 py-2 rounded-lg bg-gray-700/50 hover:bg-gray-700 text-gray-300 text-sm transition-colors"
                        >
                          Cancel
                        </button>
                      </div>
                    </div>
                  )}
                </div>

                {/* Disconnect */}
                <button
                  onClick={handleDisconnect}
                  className="w-full py-2.5 rounded-xl border border-red-500/30 hover:bg-red-500/10 text-red-400 font-medium text-sm transition-all duration-300"
                >
                  Disconnect Wallet
                </button>
              </div>
            ) : (
              /* Disconnected state */
              <div className="space-y-4">
                {/* Freighter connect */}
                <button
                  onClick={handleConnect}
                  disabled={isConnecting}
                  className="w-full group relative py-3.5 rounded-xl font-bold text-white overflow-hidden transition-all duration-300 hover:scale-[1.03] active:scale-[0.98] disabled:opacity-50 disabled:cursor-not-allowed shadow-[0_0_20px_rgba(20,184,166,0.15)] hover:shadow-[0_0_30px_rgba(20,184,166,0.4)]"
                >
                  <div className="absolute inset-0 bg-gradient-to-r from-teal-500 via-blue-600 to-indigo-600 transition-all duration-300 group-hover:bg-gradient-to-l group-hover:scale-105" />
                  <div className="absolute inset-0 opacity-0 group-hover:opacity-100 transition-opacity duration-300 bg-[radial-gradient(circle_at_center,rgba(255,255,255,0.2)_0%,transparent_100%)]" />
                  <span className="relative z-10 flex items-center justify-center gap-2">
                    {isConnecting ? (
                      <>
                        <svg
                          className="animate-spin h-5 w-5"
                          viewBox="0 0 24 24"
                        >
                          <circle
                            className="opacity-25"
                            cx="12"
                            cy="12"
                            r="10"
                            stroke="currentColor"
                            strokeWidth="4"
                            fill="none"
                          />
                          <path
                            className="opacity-75"
                            fill="currentColor"
                            d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
                          />
                        </svg>
                        Connecting...
                      </>
                    ) : (
                      <>
                        <svg
                          className="h-5 w-5"
                          viewBox="0 0 24 24"
                          fill="none"
                          stroke="currentColor"
                          strokeWidth={2}
                        >
                          <path
                            strokeLinecap="round"
                            strokeLinejoin="round"
                            d="M21 12V7H5a2 2 0 010-4h14v4"
                          />
                          <path
                            strokeLinecap="round"
                            strokeLinejoin="round"
                            d="M3 5v14a2 2 0 002 2h16v-5"
                          />
                          <path
                            strokeLinecap="round"
                            strokeLinejoin="round"
                            d="M18 12a2 2 0 100 4 2 2 0 000-4z"
                          />
                        </svg>
                        Connect Freighter
                      </>
                    )}
                  </span>
                </button>

                <p className="text-xs text-gray-500 text-center">
                  Freighter must be installed and unlocked in your browser to
                  connect.
                </p>

                {/* Network info */}
                <div className="flex items-center justify-center gap-2 text-xs text-gray-500">
                  <span className="w-2 h-2 rounded-full bg-yellow-500" />
                  <span>Stellar Testnet</span>
                </div>
              </div>
            )}
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
