export type ChessVariant = "standard" | "rapid" | "blitz" | "bullet";

export interface ChessVariantOption {
  id: ChessVariant;
  label: string;
  subtitle: string;
  description: string;
  averageGameTime: string;
  accent: string;
  badgeClassName: string;
  ringClassName: string;
}

/**
 * Preset match formats shared by the home screen and matchmaking modal.
 * Declaring them once keeps the UI cheap to render and easy to test.
 */
export const CHESS_VARIANTS: readonly ChessVariantOption[] = [
  {
    id: "standard",
    label: "Standard",
    subtitle: "Balanced queue",
    description: "Classic pacing for thoughtful play and smooth Web3 setup.",
    averageGameTime: "10-15 min",
    accent: "from-sky-500 to-cyan-400",
    badgeClassName: "bg-sky-500/15 text-sky-200 border-sky-400/30",
    ringClassName: "group-hover:border-sky-400/60 group-hover:bg-sky-500/10",
  },
  {
    id: "rapid",
    label: "Rapid",
    subtitle: "Measured pressure",
    description: "Faster than standard while still leaving room for strategy.",
    averageGameTime: "8-10 min",
    accent: "from-emerald-500 to-teal-400",
    badgeClassName: "bg-emerald-500/15 text-emerald-200 border-emerald-400/30",
    ringClassName:
      "group-hover:border-emerald-400/60 group-hover:bg-emerald-500/10",
  },
  {
    id: "blitz",
    label: "Blitz",
    subtitle: "Fast tactical swings",
    description: "High-tempo matches built for sharp decisions and quick turns.",
    averageGameTime: "3-5 min",
    accent: "from-amber-500 to-orange-400",
    badgeClassName: "bg-amber-500/15 text-amber-200 border-amber-400/30",
    ringClassName:
      "group-hover:border-amber-400/60 group-hover:bg-amber-500/10",
  },
  {
    id: "bullet",
    label: "Bullet",
    subtitle: "Pure adrenaline",
    description: "Ultra-fast games for players who want nonstop momentum.",
    averageGameTime: "< 2 min",
    accent: "from-fuchsia-500 to-pink-500",
    badgeClassName: "bg-fuchsia-500/15 text-fuchsia-200 border-fuchsia-400/30",
    ringClassName:
      "group-hover:border-fuchsia-400/60 group-hover:bg-fuchsia-500/10",
  },
] as const;

export const DEFAULT_CHESS_VARIANT: ChessVariant = "standard";

export function getChessVariantById(variant: ChessVariant): ChessVariantOption {
  return (
    CHESS_VARIANTS.find((option) => option.id === variant) ?? CHESS_VARIANTS[0]
  );
}
