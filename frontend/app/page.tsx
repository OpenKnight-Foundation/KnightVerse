"use client";

import React, { useState, useEffect } from "react";
import dynamic from "next/dynamic";
import { HeroBranding } from "@/components/HeroBranding";
import { WalletConnectModal } from "@/components/WalletConnectModal";
import { endpoints } from "@/lib/api";

const HeroChessGame = dynamic(
  () => import("@/components/chess/HeroChessGame"),
  {
    ssr: false,
    loading: () => (
      <div className="min-h-screen p-4 md:p-8">
        <div className="max-w-7xl mx-auto">
          <div className="flex flex-col lg:flex-row gap-8 items-center justify-center min-h-[80vh]">
            <div className="w-full max-w-[560px] order-2 lg:order-1">
              <div className="flex items-center justify-between mb-3 px-1">
                <div className="flex items-center gap-2">
                  <div className="w-3 h-3 rounded-full bg-white animate-pulse" />
                  <div className="h-4 w-24 bg-gray-700/50 rounded animate-pulse" />
                </div>
                <div className="flex items-center gap-2">
                  <div className="h-4 w-20 bg-gray-700/50 rounded animate-pulse" />
                  <div className="w-3 h-3 rounded-full bg-gray-600" />
                </div>
              </div>
              <div className="w-full max-w-[560px] min-w-[320px] aspect-square rounded-md border-2 border-gray-700/50 p-1">
                <div className="grid grid-cols-8 grid-rows-8 gap-0 w-full h-full">
                  {Array.from({ length: 64 }).map((_, i) => (
                    <div
                      key={i}
                      className={`${
                        (Math.floor(i / 8) + (i % 8)) % 2 === 0
                          ? "bg-gray-700/30"
                          : "bg-gray-600/20"
                      } rounded-sm shimmer-bg`}
                    />
                  ))}
                </div>
              </div>
            </div>
            <div className="w-full max-w-[280px] order-3 flex justify-center mt-4 lg:mt-0">
              <div className="w-full h-64 bg-gray-800/40 rounded-xl border border-gray-700/50 animate-pulse" />
            </div>
          </div>
        </div>
      </div>
    ),
  },
);

export default function Home() {
  const [onlinePlayerCount, setOnlinePlayerCount] = useState<number | null>(null);
  const PLAYER_COUNT_ENDPOINT = endpoints.players.online();
  const [isWalletModalOpen, setIsWalletModalOpen] = useState(false);

  useEffect(() => {
    let isMounted = true;
    const fetchOnlinePlayers = async () => {
      try {
        const resp = await fetch(PLAYER_COUNT_ENDPOINT);
        if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
        const data = (await resp.json()) as { count?: number };
        if (isMounted && typeof data.count === "number") {
          setOnlinePlayerCount(data.count);
        }
      } catch {
        console.warn("[KnightVerse] Could not fetch online player count — backend offline?");
        if (isMounted) setOnlinePlayerCount(null);
      }
    };

    fetchOnlinePlayers();
    const intervalId = window.setInterval(fetchOnlinePlayers, 20000);
    return () => {
      isMounted = false;
      clearInterval(intervalId);
    };
  }, [PLAYER_COUNT_ENDPOINT]);

  const heroBranding = (
    <HeroBranding
      onlinePlayerCount={onlinePlayerCount}
      onConnectWallet={() => setIsWalletModalOpen(true)}
    />
  );

  return (
    <>
      <HeroChessGame
        onlinePlayerCount={onlinePlayerCount}
        heroBranding={heroBranding}
      />
      <WalletConnectModal
        isOpen={isWalletModalOpen}
        onClose={() => setIsWalletModalOpen(false)}
      />
    </>
  );
}
