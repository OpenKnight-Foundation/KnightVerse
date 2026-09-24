"use client";

import React from "react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { WalletButton } from "@/components/Web3/WalletButton";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";

const navItems = [
  { href: "/", label: "Play", icon: "♟️" },
  { href: "/puzzles", label: "Puzzles", icon: "🧩" },
  { href: "/leaderboard", label: "Leaderboard", icon: "🏆" },
];

/**
 * EnhancedHeader — Premium navigation header with Web3 integration
 * Features smooth animations, active state indicators, and wallet connection
 */
export function EnhancedHeader() {
  const pathname = usePathname();

  return (
    <header className="sticky top-0 z-50 w-full border-b border-gray-800/50 bg-gray-900/80 backdrop-blur-xl">
      <div className="container mx-auto px-4">
        <div className="flex h-16 items-center justify-between">
          {/* Logo */}
          <Link
            href="/"
            className="flex items-center gap-3 group transition-transform duration-300 hover:scale-105"
          >
            <div className="relative">
              <div className="absolute inset-0 bg-gradient-to-r from-teal-500 to-blue-600 rounded-lg blur-md opacity-50 group-hover:opacity-75 transition-opacity duration-300" />
              <div className="relative bg-gradient-to-r from-teal-500 to-blue-600 p-2 rounded-lg">
                <span className="text-2xl">♔</span>
              </div>
            </div>
            <div className="flex flex-col">
              <span className="text-xl font-bold bg-gradient-to-r from-teal-400 to-blue-500 bg-clip-text text-transparent">
                XLMate
              </span>
              <span className="text-[10px] text-gray-500 -mt-1">
                Chess on Stellar
              </span>
            </div>
          </Link>

          {/* Navigation */}
          <nav className="hidden md:flex items-center gap-1" role="navigation">
            {navItems.map((item) => {
              const isActive = pathname === item.href;
              return (
                <Link
                  key={item.href}
                  href={item.href}
                  className={cn(
                    "relative px-4 py-2 rounded-lg text-sm font-medium transition-all duration-300 hover:scale-105 active:scale-95",
                    isActive
                      ? "text-white"
                      : "text-gray-400 hover:text-white hover:bg-gray-800/50"
                  )}
                >
                  {isActive && (
                    <span className="absolute inset-0 bg-gradient-to-r from-teal-500/20 to-blue-600/20 rounded-lg animate-scale-in" />
                  )}
                  <span className="relative flex items-center gap-2">
                    <span aria-hidden="true">{item.icon}</span>
                    {item.label}
                  </span>
                  {isActive && (
                    <span className="absolute bottom-0 left-1/2 -translate-x-1/2 w-8 h-0.5 bg-gradient-to-r from-teal-500 to-blue-600 rounded-full" />
                  )}
                </Link>
              );
            })}
          </nav>

          {/* Right section */}
          <div className="flex items-center gap-3">
            {/* Network indicator */}
            <Badge variant="warning" className="hidden sm:flex gap-1.5">
              <span className="w-1.5 h-1.5 rounded-full bg-yellow-400 animate-pulse" />
              Testnet
            </Badge>

            {/* Wallet button */}
            <WalletButton />
          </div>
        </div>
      </div>

      {/* Mobile navigation */}
      <nav className="md:hidden border-t border-gray-800/50 bg-gray-900/95 backdrop-blur-xl">
        <div className="container mx-auto px-4">
          <div className="flex items-center justify-around py-2">
            {navItems.map((item) => {
              const isActive = pathname === item.href;
              return (
                <Link
                  key={item.href}
                  href={item.href}
                  className={cn(
                    "flex flex-col items-center gap-1 px-4 py-2 rounded-lg text-xs font-medium transition-all duration-300",
                    isActive
                      ? "text-white bg-gradient-to-r from-teal-500/20 to-blue-600/20"
                      : "text-gray-400 hover:text-white"
                  )}
                >
                  <span className="text-lg" aria-hidden="true">
                    {item.icon}
                  </span>
                  {item.label}
                </Link>
              );
            })}
          </div>
        </div>
      </nav>
    </header>
  );
}
