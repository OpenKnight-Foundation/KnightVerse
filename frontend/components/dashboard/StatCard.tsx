"use client";

import type { ReactNode } from "react";
import { ArrowDownRight, ArrowUpRight } from "lucide-react";
import { cn } from "@/lib/utils";

interface StatCardProps {
  icon: ReactNode;
  label: string;
  value: string | number;
  trend?: { value: number; label: string };
  accentColor?: string;
}

const accentStyles: Record<string, { icon: string; ring: string; glow: string }> = {
  teal: {
    icon: "bg-gradient-to-br from-teal-400/20 to-teal-500/10 text-teal-300",
    ring: "border-teal-500/20",
    glow: "from-teal-500/10 to-blue-500/5",
  },
  blue: {
    icon: "bg-gradient-to-br from-blue-400/20 to-indigo-500/10 text-blue-300",
    ring: "border-blue-500/20",
    glow: "from-blue-500/10 to-indigo-500/5",
  },
  emerald: {
    icon: "bg-gradient-to-br from-emerald-400/20 to-emerald-500/10 text-emerald-300",
    ring: "border-emerald-500/20",
    glow: "from-emerald-500/10 to-teal-500/5",
  },
  amber: {
    icon: "bg-gradient-to-br from-amber-400/20 to-orange-500/10 text-amber-300",
    ring: "border-amber-500/20",
    glow: "from-amber-500/10 to-orange-500/5",
  },
  rose: {
    icon: "bg-gradient-to-br from-rose-400/20 to-red-500/10 text-rose-300",
    ring: "border-rose-500/20",
    glow: "from-rose-500/10 to-red-500/5",
  },
};

export default function StatCard({
  icon,
  label,
  value,
  trend,
  accentColor = "teal",
}: StatCardProps) {
  const accent = accentStyles[accentColor] ?? accentStyles.teal;
  const trendPositive = (trend?.value ?? 0) >= 0;

  return (
    <article
      role="article"
      aria-label={label}
      className={cn(
        "animate-scale-in rounded-xl border border-gray-700/30 bg-gray-800/40 p-5 shadow-lg shadow-black/10",
        accent.ring,
      )}
    >
      <div className={cn("mb-4 inline-flex rounded-xl p-3", accent.icon)}>{icon}</div>
      <div className="space-y-2">
        <p className="text-sm uppercase tracking-[0.18em] text-gray-400">{label}</p>
        <div className="flex items-end justify-between gap-3">
          <p className="text-3xl font-bold text-white">{value}</p>
          {trend ? (
            <div
              className={cn(
                "inline-flex items-center gap-1 rounded-full border px-2.5 py-1 text-xs font-semibold",
                trendPositive
                  ? "border-emerald-500/30 bg-emerald-500/10 text-emerald-400"
                  : "border-red-500/30 bg-red-500/10 text-red-400",
              )}
            >
              {trendPositive ? <ArrowUpRight className="h-3.5 w-3.5" /> : <ArrowDownRight className="h-3.5 w-3.5" />}
              <span>{trendPositive ? "+" : ""}{trend.value.toFixed(1)}</span>
              <span className="text-gray-300">{trend.label}</span>
            </div>
          ) : null}
        </div>
      </div>
      <div className={cn("mt-4 h-px rounded-full bg-gradient-to-r", accent.glow)} />
    </article>
  );
}
