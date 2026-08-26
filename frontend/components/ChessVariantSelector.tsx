"use client";

import React from "react";
import { type ChessVariant, CHESS_VARIANTS } from "@/lib/chessVariants";
import { cn } from "@/lib/utils";

interface ChessVariantSelectorProps {
  selectedVariant: ChessVariant;
  onSelect: (variant: ChessVariant) => void;
}

/**
 * Lets players choose the chess format before creating a bot or online match.
 */
export function ChessVariantSelector({
  selectedVariant,
  onSelect,
}: ChessVariantSelectorProps) {
  return (
    <section
      aria-labelledby="match-variant-heading"
      className="rounded-[28px] border border-white/10 bg-[radial-gradient(circle_at_top,rgba(34,211,238,0.12),transparent_42%),linear-gradient(180deg,rgba(17,24,39,0.95),rgba(3,7,18,0.98))] p-5 shadow-[0_24px_80px_rgba(2,132,199,0.12)] backdrop-blur-xl"
    >
      <div className="mb-4 flex items-start justify-between gap-4">
        <div>
          <p className="text-[11px] font-semibold uppercase tracking-[0.32em] text-cyan-300/80">
            Match Setup
          </p>
          <h2
            id="match-variant-heading"
            className="mt-1 text-xl font-semibold text-white"
          >
            Choose your chess variant
          </h2>
        </div>
        <span className="rounded-full border border-cyan-400/25 bg-cyan-500/10 px-3 py-1 text-[11px] font-medium uppercase tracking-[0.24em] text-cyan-100">
          Web3 Ready
        </span>
      </div>

      <div className="grid gap-3 sm:grid-cols-2">
        {CHESS_VARIANTS.map((variant) => {
          const isSelected = variant.id === selectedVariant;

          return (
            <button
              key={variant.id}
              type="button"
              onClick={() => onSelect(variant.id)}
              aria-pressed={isSelected}
              className={cn(
                "group relative overflow-hidden rounded-2xl border px-4 py-4 text-left transition-all duration-300",
                "border-white/10 bg-white/[0.03] hover:-translate-y-0.5 hover:border-white/20",
                "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cyan-300/70 focus-visible:ring-offset-2 focus-visible:ring-offset-slate-950",
                isSelected &&
                  "border-cyan-300/60 bg-white/[0.07] shadow-[0_0_0_1px_rgba(103,232,249,0.25),0_20px_50px_rgba(34,211,238,0.15)]",
              )}
            >
              <div
                className={cn(
                  "pointer-events-none absolute inset-0 transition-opacity duration-300",
                  `bg-gradient-to-br ${variant.accent}`,
                  isSelected ? "opacity-20" : "opacity-0 group-hover:opacity-15",
                )}
              />

              <div className="relative flex items-start justify-between gap-4">
                <div>
                  <div className="flex items-center gap-2">
                    <h3 className="text-base font-semibold text-white">
                      {variant.label}
                    </h3>
                    <span
                      className={cn(
                        "rounded-full border px-2.5 py-1 text-[10px] font-semibold uppercase tracking-[0.24em]",
                        variant.badgeClassName,
                      )}
                    >
                      {variant.averageGameTime}
                    </span>
                  </div>
                  <p className="mt-1 text-sm font-medium text-cyan-100/85">
                    {variant.subtitle}
                  </p>
                  <p className="mt-2 text-sm leading-6 text-slate-300">
                    {variant.description}
                  </p>
                </div>

                <span
                  className={cn(
                    "mt-1 flex h-6 w-6 flex-shrink-0 items-center justify-center rounded-full border transition-all duration-300",
                    isSelected
                      ? "border-cyan-300 bg-cyan-300/15"
                      : "border-white/15 bg-white/5",
                    !isSelected && variant.ringClassName,
                  )}
                  aria-hidden="true"
                >
                  <span
                    className={cn(
                      "h-2.5 w-2.5 rounded-full transition-all duration-300",
                      isSelected ? "bg-cyan-200" : "bg-transparent",
                    )}
                  />
                </span>
              </div>
            </button>
          );
        })}
      </div>
    </section>
  );
}
