"use client";

import { Shield, Sparkles } from "lucide-react";
import { getRankTier } from "@/lib/eloStatsUtils";
import { cn } from "@/lib/utils";

interface RankBadgeProps {
  /** The player's current rating. */
  elo: number;
}

// Tailwind classes for each rank-tier color, keyed by the color name that
// `getRankTier` returns. Each entry styles three parts of the badge:
//   shell - gradient background, border and text color of the card
//   icon  - color of the shield icon
//   bar   - gradient fill of the progress bar
// Full class strings are written out (not built dynamically) so Tailwind can
// detect them at build time.
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

/**
 * Card showing the player's rank tier, their rating, and a progress bar
 * toward the next tier. The top tier has no upper limit, so it shows a
 * full bar and a "Top tier" label instead.
 */
export default function RankBadge({ elo }: RankBadgeProps) {
  // Look up which tier this rating falls into (name, color, min/max rating)
  const tier = getRankTier(elo);
  // Fall back to the indigo style if the tier's color has no entry above
  const styles = tierStyles[tier.color] ?? tierStyles.indigo;
  // The highest tier has no finite upper bound (maxElo is Infinity)
  const isTopTier = !Number.isFinite(tier.maxElo);
  // Percentage of the way through the current tier's rating range.
  // The `+ 1` makes the range inclusive of maxElo, so the bar reaches 100%
  // only when the player moves up to the next tier. Clamped to 0-100 in case
  // the rating is somehow outside the tier's range.
  const progress = isTopTier
    ? 100
    : Math.min(100, Math.max(0, ((elo - tier.minElo) / (tier.maxElo - tier.minElo + 1)) * 100));
  // Points needed to reach the first rating of the next tier (never negative)
  const remaining = isTopTier ? 0 : Math.max(0, tier.maxElo + 1 - elo);

  return (
    // `role="status"` lets screen readers announce the badge when it updates
    <section
      role="status"
      aria-label={`Current rank tier ${tier.name}`}
      className="rounded-xl border border-gray-700/30 bg-gray-800/40 p-5 shadow-lg shadow-black/10"
    >
      {/* Inner card: gradient, border and text colors come from the tier's `shell` style */}
      <div className={cn("rounded-2xl border bg-gradient-to-br p-4", styles.shell)}>
        {/* Top row: icon and tier name on the left, rating pill on the right */}
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
          {/* Current rating */}
          <div className="inline-flex items-center gap-2 rounded-full border border-white/10 bg-black/20 px-3 py-1.5 text-sm text-gray-100">
            <Sparkles className="h-4 w-4" />
            {elo}
          </div>
        </div>

        {/* Progress toward the next tier */}
        <div className="mt-4">
          <div className="mb-2 flex items-center justify-between text-xs uppercase tracking-[0.16em] text-gray-300">
            <span>Progress</span>
            <span>{isTopTier ? "Top tier" : `${remaining} pts to next`}</span>
          </div>
          {/* Dark track with a gradient fill whose width is the progress percentage */}
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