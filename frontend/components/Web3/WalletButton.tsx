"use client";

import React, { useState } from "react";
import { useAppContext } from "@/context/walletContext";
import { WalletConnectModal } from "@/components/WalletConnectModal";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";

/**
 * WalletButton — Premium wallet connection button with status indicator
 * Displays connection status and opens wallet modal on click
 */
export function WalletButton() {
  const { address, status } = useAppContext();
  const [isModalOpen, setIsModalOpen] = useState(false);

  const truncateAddress = (addr: string) =>
    `${addr.slice(0, 4)}...${addr.slice(-4)}`;

  const statusConfig = {
    connected: {
      variant: "success" as const,
      label: "Connected",
      showAddress: true,
    },
    connecting: {
      variant: "warning" as const,
      label: "Connecting...",
      showAddress: false,
    },
    disconnected: {
      variant: "outline" as const,
      label: "Connect Wallet",
      showAddress: false,
    },
    error: {
      variant: "destructive" as const,
      label: "Connection Error",
      showAddress: false,
    },
  };

  const config = statusConfig[status] || statusConfig.disconnected;

  return (
    <>
      <Button
        onClick={() => setIsModalOpen(true)}
        variant={address ? "outline" : "default"}
        className="group relative overflow-hidden transition-all duration-300 hover:scale-[1.02] active:scale-[0.98]"
        aria-label={address ? `Wallet connected: ${truncateAddress(address)}` : "Connect wallet"}
      >
        {!address && (
          <div className="absolute inset-0 bg-gradient-to-r from-teal-500/20 to-blue-600/20 opacity-0 group-hover:opacity-100 transition-opacity duration-300" />
        )}
        <div className="relative flex items-center gap-2">
          {/* Status indicator dot */}
          <span
            className={`w-2 h-2 rounded-full ${
              status === "connected"
                ? "bg-emerald-400 animate-pulse-glow"
                : status === "connecting"
                  ? "bg-yellow-400 animate-pulse"
                  : status === "error"
                    ? "bg-red-400"
                    : "bg-gray-500"
            }`}
            aria-hidden="true"
          />
          
          {/* Wallet icon */}
          <svg
            className="h-4 w-4"
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

          {/* Address or status label */}
          <span className="font-medium">
            {config.showAddress && address
              ? truncateAddress(address)
              : config.label}
          </span>

          {/* Network badge */}
          {address && (
            <Badge variant="warning" className="ml-1 text-[10px] px-1.5 py-0">
              Testnet
            </Badge>
          )}
        </div>
      </Button>

      <WalletConnectModal
        isOpen={isModalOpen}
        onClose={() => setIsModalOpen(false)}
      />
    </>
  );
}
