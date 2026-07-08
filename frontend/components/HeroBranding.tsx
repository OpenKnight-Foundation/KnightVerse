"use client";

import React from "react";
import Image from "next/image";
import { useAppContext } from "@/context/walletContext";
import { FaWallet, FaBolt, FaShieldAlt, FaChessKnight } from "react-icons/fa";
import { RiAliensFill } from "react-icons/ri";

interface HeroBrandingProps {
  onlinePlayerCount: number | null;
  onConnectWallet: () => void;
}

export function HeroBranding({ onlinePlayerCount, onConnectWallet }: HeroBrandingProps) {
  const { address, status } = useAppContext();

  const truncateAddress = (addr: string) =>
    `${addr.slice(0, 6)}...${addr.slice(-4)}`;

  return (
    <div className="flex flex-col gap-6 w-full max-w-md">
      {/* Logo + Tagline */}
      <div className="flex flex-col items-center md:items-start gap-3 animate-fade-in">
        <div className="relative w-20 h-20 animate-float">
          <Image
            src="/images/KnightVerseLogo.png"
            alt="KnightVerse"
            fill
            className="object-contain drop-shadow-2xl"
            priority
          />
        </div>
        <div className="text-center md:text-left">
          <h1 className="text-4xl md:text-5xl font-bold bg-gradient-to-r from-teal-300 via-cyan-400 to-blue-500 bg-clip-text text-transparent tracking-tight">
            KnightVerse
          </h1>
          <p className="text-gray-400 text-sm mt-1 tracking-wide">
            Decentralized Chess on Stellar
          </p>
        </div>
      </div>

      {/* Play Now banner */}
      <div className="bg-gradient-to-r from-teal-500/10 to-blue-600/10 border border-teal-500/20 rounded-2xl p-4 animate-slide-up">
        <div className="flex items-center gap-3 mb-2">
          <div className="w-3 h-3 rounded-full bg-emerald-400 animate-pulse-glow" />
          <span className="text-emerald-400 font-semibold text-sm uppercase tracking-widest">
            Live — Play Now
          </span>
        </div>
        <p className="text-gray-300 text-sm leading-relaxed">
          Move a piece to start! You play as <span className="text-white font-semibold">White</span>,
          the AI responds as <span className="text-gray-400 font-semibold">Black</span>.
        </p>
      </div>

      {/* Stellar Network Status + Wallet */}
      <div className="flex flex-col gap-3 animate-slide-up" style={{ animationDelay: "0.1s" }}>
        {/* Network Pill */}
        <div className="flex items-center gap-2">
          <div className="flex items-center gap-1.5 px-3 py-1.5 rounded-full bg-gray-800/60 border border-gray-700/40 text-xs">
            <span className="w-1.5 h-1.5 rounded-full bg-yellow-500" />
            <span className="text-gray-400">Stellar Testnet</span>
          </div>
          {onlinePlayerCount !== null && (
            <div className="flex items-center gap-1.5 px-3 py-1.5 rounded-full bg-gray-800/60 border border-gray-700/40 text-xs">
              <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse" />
              <span className="text-gray-400">{onlinePlayerCount} online</span>
            </div>
          )}
        </div>

        {/* Connect Wallet Button */}
        {status === "connected" && address ? (
          <div className="flex items-center gap-3 p-3 rounded-xl bg-gray-800/60 border border-gray-700/40">
            <div className="w-9 h-9 rounded-full bg-gradient-to-br from-teal-400 to-blue-600 flex items-center justify-center text-white text-xs font-bold flex-shrink-0">
              {address.slice(0, 2).toUpperCase()}
            </div>
            <div className="flex-1 min-w-0">
              <p className="text-sm font-medium text-white truncate">
                {truncateAddress(address)}
              </p>
              <div className="flex items-center gap-1.5 mt-0.5">
                <span className="w-2 h-2 rounded-full bg-emerald-400 animate-pulse-glow" />
                <span className="text-xs text-emerald-400">Connected</span>
              </div>
            </div>
          </div>
        ) : (
          <button
            onClick={onConnectWallet}
            className="w-full flex items-center justify-center gap-2 px-5 py-3 bg-gradient-to-r from-teal-500 to-blue-600 hover:from-teal-400 hover:to-blue-500 text-white font-semibold rounded-xl shadow-lg shadow-teal-500/20 hover:shadow-teal-400/30 transition-all duration-300 hover:scale-[1.02] active:scale-[0.98]"
          >
            <FaWallet className="text-lg" />
            Connect Stellar Wallet
          </button>
        )}
      </div>

      {/* Feature Highlights */}
      <div className="grid grid-cols-2 gap-3 animate-slide-up" style={{ animationDelay: "0.2s" }}>
        <FeatureCard
          icon={<FaChessKnight className="text-teal-400" />}
          title="Play Instantly"
          desc="No signup required"
        />
        <FeatureCard
          icon={<RiAliensFill className="text-purple-400" />}
          title="AI Co-pilots"
          desc="Stockfish & Leela"
        />
        <FeatureCard
          icon={<FaBolt className="text-yellow-400" />}
          title="Ultra Low Fees"
          desc="~0.00001 XLM/tx"
        />
        <FeatureCard
          icon={<FaShieldAlt className="text-blue-400" />}
          title="On-chain"
          desc="Soroban contracts"
        />
      </div>

      {/* Scroll hint */}
      <div className="hidden md:flex items-center justify-center gap-2 text-gray-500 text-xs animate-fade-in mt-2" style={{ animationDelay: "0.4s" }}>
        <span>Scroll for more game modes</span>
        <svg className="w-4 h-4 animate-bounce" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
          <path strokeLinecap="round" strokeLinejoin="round" d="M19 14l-7 7m0 0l-7-7m7 7V3" />
        </svg>
      </div>
    </div>
  );
}

function FeatureCard({ icon, title, desc }: { icon: React.ReactNode; title: string; desc: string }) {
  return (
    <div className="flex items-start gap-2.5 p-3 rounded-xl bg-gray-800/40 border border-gray-700/30 hover:border-gray-600/50 transition-colors duration-300 group">
      <div className="text-lg mt-0.5 group-hover:scale-110 transition-transform duration-300">
        {icon}
      </div>
      <div>
        <p className="text-white text-sm font-medium leading-tight">{title}</p>
        <p className="text-gray-500 text-xs mt-0.5">{desc}</p>
      </div>
    </div>
  );
}

export default HeroBranding;
