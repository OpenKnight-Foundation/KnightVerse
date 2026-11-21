"use client";

import React, { useRef, useState } from "react";
import Image from "next/image";
import { cn } from "@/lib/utils";

export interface AICompanion {
  id: string;
  name: string;
  level: number;
  exp: number;
  tacticalStyle: "Aggressive" | "Defensive" | "Tactical" | "Endgame Master";
  winRate: number;
  winStreak: number;
  image: string;
  priceXLM: number;
}

export interface CompanionCard3DProps {
  companion: AICompanion;
  onMint: (companionId: string) => void;
  onRent: (companionId: string) => void;
  disabled?: boolean;
  isMintDisabled?: boolean;
  isRentDisabled?: boolean;
  mintLabel?: string;
  rentLabel?: string;
  isMinting?: boolean;
}

export const CompanionCard3D: React.FC<CompanionCard3DProps> = ({
  companion,
  onMint,
  onRent,
  disabled = false,
  isMintDisabled,
  isRentDisabled,
  mintLabel,
  rentLabel,
  isMinting = false,
}) => {
  const cardRef = useRef<HTMLDivElement>(null);
  const [rotateX, setRotateX] = useState(0);
  const [rotateY, setRotateY] = useState(0);
  const [glareStyle, setGlareStyle] = useState({ x: 50, y: 50, opacity: 0 });
  const rafId = useRef<number | null>(null);

  const rentDisabled = disabled || (isRentDisabled ?? false);
  const mintDisabled = disabled || (isMintDisabled ?? false) || isMinting;

  const handleMouseMove = (e: React.MouseEvent<HTMLDivElement>) => {
    if (!cardRef.current) return;
    const rect = cardRef.current.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;

    const centerX = rect.width / 2;
    const centerY = rect.height / 2;

    const rX = -((y - centerY) / centerY) * 15;
    const rY = ((x - centerX) / centerX) * 15;

    if (rafId.current) cancelAnimationFrame(rafId.current);

    rafId.current = requestAnimationFrame(() => {
      setRotateX(rX);
      setRotateY(rY);
      setGlareStyle({
        x: (x / rect.width) * 100,
        y: (y / rect.height) * 100,
        opacity: 0.25,
      });
    });
  };

  const handleMouseLeave = () => {
    if (rafId.current) cancelAnimationFrame(rafId.current);
    setRotateX(0);
    setRotateY(0);
    setGlareStyle((prev) => ({ ...prev, opacity: 0 }));
  };

  return (
    <div
      style={{ perspective: "1000px" }}
      className="w-full max-w-sm cursor-pointer select-none"
    >
      <div
        ref={cardRef}
        onMouseMove={handleMouseMove}
        onMouseLeave={handleMouseLeave}
        style={{
          transform: `rotateX(${rotateX}deg) rotateY(${rotateY}deg)`,
          transition:
            rotateX === 0 && rotateY === 0 ? "transform 0.5s ease-out" : "none",
          transformStyle: "preserve-3d",
        }}
        className="relative rounded-2xl border border-slate-700 bg-slate-900/80 p-5 shadow-2xl backdrop-blur-md overflow-hidden text-white"
      >
        <div
          className="absolute inset-0 pointer-events-none transition-opacity duration-300"
          style={{
            opacity: glareStyle.opacity,
            background: `radial-gradient(circle at ${glareStyle.x}% ${glareStyle.y}%, rgba(255,255,255,0.4) 0%, transparent 80%)`,
          }}
        />

        <div className="flex items-center justify-between mb-4">
          <span className="px-3 py-1 text-xs font-semibold uppercase tracking-wider bg-indigo-500/20 text-indigo-400 rounded-full border border-indigo-500/30">
            {companion.tacticalStyle}
          </span>
          <span className="text-sm font-mono text-slate-400">
            Mint ID: #{companion.id}
          </span>
        </div>

        <div className="relative h-48 w-full rounded-xl overflow-hidden mb-4 bg-slate-800 border border-slate-700/50 flex items-center justify-center">
          <Image
            src={companion.image}
            alt={companion.name}
            fill
            className="object-cover transform scale-105 hover:scale-110 transition-transform duration-500"
          />
          <div className="absolute bottom-2 left-2 bg-black/60 backdrop-blur-sm px-2.5 py-1 rounded-lg text-xs font-medium">
            Win Rate: {companion.winRate}%
          </div>
        </div>

        <h3 className="text-xl font-bold mb-1 tracking-tight">
          {companion.name}
        </h3>
        <p className="text-xs text-slate-400 mb-4">
          Level {companion.level} • {companion.winStreak} Win Streak
        </p>

        <div className="mb-5">
          <div className="flex justify-between text-xs font-medium text-slate-400 mb-1">
            <span>EXP Progress</span>
            <span>{companion.exp}%</span>
          </div>
          <div className="h-2 w-full bg-slate-800 rounded-full overflow-hidden border border-slate-700/50">
            <div
              className="h-full bg-gradient-to-r from-indigo-500 to-cyan-400 rounded-full transition-all duration-300"
              style={{ width: `${companion.exp}%` }}
            />
          </div>
        </div>

        <div className="grid grid-cols-2 gap-3">
          <button
            type="button"
            disabled={rentDisabled}
            aria-disabled={rentDisabled}
            onClick={() => {
              if (!rentDisabled) onRent(companion.id);
            }}
            className={cn(
              "w-full py-2.5 px-4 rounded-xl font-semibold text-sm border transition-colors",
              rentDisabled
                ? "bg-slate-900/60 text-slate-500 border-slate-800/80 cursor-not-allowed hover:bg-slate-900/60"
                : "bg-slate-800 hover:bg-slate-700 text-slate-200 border-slate-600/50"
            )}
          >
            {rentLabel ?? (rentDisabled ? "Rent (Coming Soon)" : "Rent Agent")}
          </button>
          <button
            type="button"
            disabled={mintDisabled}
            aria-disabled={mintDisabled}
            onClick={() => {
              if (!mintDisabled) onMint(companion.id);
            }}
            className={cn(
              "w-full py-2.5 px-4 rounded-xl font-semibold text-sm transition-colors",
              mintDisabled
                ? "bg-indigo-950/40 text-indigo-300/40 border border-indigo-900/30 cursor-not-allowed shadow-none hover:bg-indigo-950/40"
                : "bg-indigo-600 hover:bg-indigo-500 text-white shadow-lg shadow-indigo-600/30 border border-transparent"
            )}
          >
            {isMinting
              ? "Minting..."
              : (mintLabel ?? `Mint (${companion.priceXLM} XLM)`)}
          </button>
        </div>
      </div>
    </div>
  );
};
