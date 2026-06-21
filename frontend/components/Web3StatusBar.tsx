"use client";

import React from "react";
import { useAppContext } from "@/context/walletContext";
import { useTransactionContext } from "@/context/transactionContext";

/**
 * Web3StatusBar — A compact indicator shown at the top of the main content area.
 * Displays wallet connection status, network, and a truncated address when connected.
 * CPU-efficient: re-renders only when wallet context values change.
 */
export function Web3StatusBar() {
  const { address, status } = useAppContext();
  const { activeCount } = useTransactionContext();

  const statusConfig: Record<
    string,
    { dot: string; label: string; color: string }
  > = {
    connected: {
      dot: "bg-emerald-400 animate-pulse-glow",
      label: "Connected",
      color: "text-emerald-400",
    },
    connecting: {
      dot: "bg-yellow-400 animate-pulse",
      label: "Connecting",
      color: "text-yellow-400",
    },
    disconnected: {
      dot: "bg-gray-500",
      label: "Not Connected",
      color: "text-gray-400",
    },
    error: {
      dot: "bg-red-400",
      label: "Error",
      color: "text-red-400",
    },
  };

  const cfg = statusConfig[status] ?? statusConfig.disconnected;

  const truncateAddress = (addr: string) =>
    `${addr.slice(0, 6)}...${addr.slice(-4)}`;

  return (
    <div className="group relative flex items-center gap-3 px-5 py-2.5 rounded-full bg-gray-900/60 backdrop-blur-md border border-gray-700/50 shadow-lg hover:shadow-xl hover:border-gray-600/50 transition-all duration-300 text-sm cursor-default" role="status" aria-label="Wallet connection status">
      <div className="absolute inset-0 rounded-full bg-gradient-to-r from-teal-500/10 to-blue-500/10 opacity-0 group-hover:opacity-100 transition-opacity duration-300" />
      
      <span className="relative flex h-2.5 w-2.5 items-center justify-center">
        {status === "connected" && <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-emerald-400 opacity-75"></span>}
        <span className={`relative inline-flex h-2.5 w-2.5 rounded-full ${cfg.dot}`} aria-hidden="true" />
      </span>
      <span className={`relative ${cfg.color} font-semibold tracking-wide drop-shadow-sm`}>{cfg.label}</span>
      {address && (
        <>
          <span className="relative text-gray-600">|</span>
          <span className="relative font-mono text-gray-300 text-xs font-medium bg-gray-800/50 px-2 py-0.5 rounded-md border border-gray-700/30 shadow-inner">
            {truncateAddress(address)}
          </span>
        </>
      )}
      <span className="relative text-gray-600">|</span>
      <span className="relative text-xs text-gray-400 font-medium flex items-center gap-1.5 bg-yellow-500/10 text-yellow-200/90 px-2.5 py-0.5 rounded-md border border-yellow-500/20 transition-all hover:bg-yellow-500/20">
        <span className="w-1.5 h-1.5 rounded-full bg-yellow-500 shadow-[0_0_8px_rgba(234,179,8,0.8)]" aria-hidden="true" />
        Testnet
      </span>
      {activeCount > 0 && (
        <>
          <span className="relative text-gray-600">|</span>
          <span className="relative text-xs flex items-center gap-1.5 bg-blue-500/10 text-blue-300 px-2.5 py-0.5 rounded-md border border-blue-500/20 shadow-[0_0_10px_rgba(59,130,246,0.15)] transition-all">
            <span className="w-1.5 h-1.5 rounded-full bg-blue-400 animate-pulse shadow-[0_0_8px_rgba(96,165,250,0.8)]" aria-hidden="true" />
            {activeCount} tx active
          </span>
        </>
      )}
    </div>
  );
}
