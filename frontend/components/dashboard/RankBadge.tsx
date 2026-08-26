"use client";

import { Shield, Sparkles } from "lucide-react";
import { getRankTier } from "@/lib/eloStatsUtils";
import { cn } from "@/lib/utils";

interface RankBadgeProps {
  elo: number;
}

const tierStyles: Record<string, { shell: string; icon: string; bar: string }> = {
  slate: {
    shell: "from-slate-500/25 to-slate-300/10 border-slate-400/25 text-slate-100",
    icon: "text-slate-300",
    bar: "from-slate-400 to-slate-200",
  },
  teal: {
    shell: "from-teal-500/25 to-cyan-400/10 border-teal-400/25 text-teal-50",
    icon: "text-teal-300",
    bar: "from-teal-400 to-cyan-300",
  },
  indigo: {
    shell: "from-indigo-500/25 to-blue-400/10 border-indigo-400/25 text-indigo-50",
    icon: "text-indigo-300",
    bar: "from-indigo-400 to-blue-300",
  },
  purple: {
    shell: "from-purple-500/25 to-fuchsia-400/10 border-purple-400/25 text-purple-50",
    icon: "text-purple-300",
    bar: "from-purple-400 to-fuchsia-300",
  },
  amber: {
    shell: "from-amber-500/25 to-orange-400/10 border-amber-400/25 text-amber-50",
    icon: "text-amber-300",
    bar: "from-amber-400 to-orange-300",
  },
  rose: {
    shell: "from-rose-500/25 to-red-400/10 border-rose-400/25 text-rose-50",
    icon: "text-rose-300",
    bar: "from-rose-400 to-red-300",
  },
};

export default function RankBadge({ elo }: RankBadgeProps) {
  const tier = getRankTier(elo);
  const styles = tierStyles[tier.color] ?? tierStyles.indigo;
  const isTopTier = !Number.isFinite(tier.maxElo);
  const progress = isTopTier
    ? 100
    : Math.min(100, Math.max(0, ((elo - tier.minElo) / (tier.maxElo - tier.minElo + 1)) * 100));
  const remaining = isTopTier ? 0 : Math.max(0, tier.maxElo + 1 - elo);

  return (
    <section
      role="status"
      aria-label={`Current rank tier ${tier.name}`}
      className="rounded-xl border border-gray-700/30 bg-gray-800/40 p-5 shadow-lg shadow-black/10"
    >
      <div className={cn("rounded-2xl border bg-gradient-to-br p-4", styles.shell)}>
        <div className="flex items-center justify-between gap-4">
          <div className="flex items-center gap-3">
            <div className="rounded-2xl bg-black/20 p-3 backdrop-blur-sm">
              <Shield className={cn("h-6 w-6", styles.icon)} />
            </div>
            <div>
              <p className="text-xs uppercase tracking-[0.18em] text-gray-300">Rank tier</p>
              <p className="mt-1 text-xl font-semibold text-white">{tier.name}</p>
            </div>
          </div>
          <div className="inline-flex items-center gap-2 rounded-full border border-white/10 bg-black/20 px-3 py-1.5 text-sm text-gray-100">
            <Sparkles className="h-4 w-4" />
            {elo}
          </div>
        </div>

        <div className="mt-4">
          <div className="mb-2 flex items-center justify-between text-xs uppercase tracking-[0.16em] text-gray-300">
            <span>Progress</span>
            <span>{isTopTier ? "Top tier" : `${remaining} pts to next`}</span>
          </div>
          <div className="h-2 overflow-hidden rounded-full bg-black/25">
            <div
              className={cn("h-full rounded-full bg-gradient-to-r", styles.bar)}
              style={{ width: `${progress}%` }}
            />
          </div>
        </div>
      </div>
    </section>
  );
}
