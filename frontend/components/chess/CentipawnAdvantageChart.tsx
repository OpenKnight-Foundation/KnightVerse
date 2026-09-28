"use client";

import React, { useCallback, useMemo } from "react";
import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ReferenceLine,
  ResponsiveContainer,
} from "recharts";
import type { DotItemDotProps, MouseHandlerDataParam } from "recharts";

// ── Types ─────────────────────────────────────────────────────────────────────

export type MoveQuality = "brilliant" | "best" | "good" | "inaccuracy" | "mistake" | "blunder";

export interface EvaluatedPly {
  /** 0-based ply index (0 = starting position before move 1). */
  plyIndex: number;
  /** Centipawn evaluation from White's perspective, clamped to ±1000. */
  evaluation: number | null;
  /** Mate-in-N (positive = White wins, negative = Black wins). */
  mate?: number | null;
  /** SAN of the move that led to this position (undefined for ply 0). */
  san?: string;
  /** Best engine move for this position. */
  bestMove?: string;
  /** Quality classification of the played move. */
  quality?: MoveQuality;
}

interface CentipawnAdvantageChartProps {
  /** Evaluated plies including starting position (plyIndex 0) and each move. */
  plies: EvaluatedPly[];
  /** Currently active ply index (0 = start). */
  activePly: number;
  /** Called when the user clicks a data point to scrub to that ply. */
  onPlySelect: (plyIndex: number) => void;
  /** Optional CSS class for the container element. */
  className?: string;
}

// ── Constants ─────────────────────────────────────────────────────────────────

/** Chart Y-axis domain in pawns (not centipawns). */
const Y_DOMAIN: [number, number] = [-10, 10];

const QUALITY_COLORS: Record<MoveQuality, string> = {
  brilliant: "#06b6d4",  // cyan-500
  best:      "#22c55e",  // green-500
  good:      "#84cc16",  // lime-500
  inaccuracy:"#eab308",  // yellow-500
  mistake:   "#f97316",  // orange-500
  blunder:   "#ef4444",  // red-500
};

const QUALITY_LABELS: Record<MoveQuality, string> = {
  brilliant:  "Brilliant ✨",
  best:       "Best Move ✓",
  good:       "Good Move",
  inaccuracy: "Inaccuracy ?!",
  mistake:    "Mistake ?",
  blunder:    "Blunder ??",
};

// ── Helpers ───────────────────────────────────────────────────────────────────

/**
 * Convert centipawn evaluation to a pawn value clamped to ±10 for the chart.
 * Mate scores map to ±10.
 */
function cpToPawns(cp: number | null, mate?: number | null): number {
  if (mate != null) return mate > 0 ? 10 : -10;
  if (cp == null) return 0;
  return Math.max(-10, Math.min(10, cp / 100));
}

/** Format the evaluation value for display in the tooltip. */
function formatEval(cp: number | null, mate?: number | null): string {
  if (mate != null) return `M${Math.abs(mate)}`;
  if (cp == null) return "0.00";
  const pawns = cp / 100;
  return pawns >= 0 ? `+${pawns.toFixed(2)}` : pawns.toFixed(2);
}

/** Return the move number label for a ply. */
function plyToMoveLabel(plyIndex: number): string {
  if (plyIndex === 0) return "Start";
  const moveNum = Math.ceil(plyIndex / 2);
  const side = plyIndex % 2 === 1 ? "W" : "B";
  return `${moveNum}${side}`;
}

// ── Custom dot renderer ───────────────────────────────────────────────────────

interface CustomDotProps {
  cx?: number;
  cy?: number;
  payload?: EvaluatedPly & { pawns: number };
  activePly: number;
  onPlySelect: (plyIndex: number) => void;
}

function QualityDot({ cx = 0, cy = 0, payload, activePly, onPlySelect }: CustomDotProps) {
  if (!payload) return null;

  const isActive = payload.plyIndex === activePly;
  const quality = payload.quality;

  // Only render a visible dot for annotated plies or the active ply
  if (!quality && !isActive) return null;

  const color = quality ? QUALITY_COLORS[quality] : "#6366f1"; // indigo for active
  const r = isActive ? 7 : 5;

  return (
    <circle
      cx={cx}
      cy={cy}
      r={r}
      fill={color}
      stroke={isActive ? "#fff" : color}
      strokeWidth={isActive ? 2 : 1}
      style={{ cursor: "pointer", filter: isActive ? "drop-shadow(0 0 4px white)" : undefined }}
      onClick={() => onPlySelect(payload.plyIndex)}
      role="button"
      aria-label={`Go to ply ${payload.plyIndex}${payload.san ? `, move ${payload.san}` : ""}`}
    />
  );
}

// ── Custom tooltip ────────────────────────────────────────────────────────────

interface TooltipPayloadItem {
  payload: EvaluatedPly & { pawns: number };
}

interface CustomTooltipProps {
  active?: boolean;
  payload?: TooltipPayloadItem[];
}

function ChartTooltip({ active, payload }: CustomTooltipProps) {
  if (!active || !payload?.length) return null;
  const d = payload[0].payload;

  const evalLabel = formatEval(d.evaluation, d.mate);
  const qualityColor = d.quality ? QUALITY_COLORS[d.quality] : undefined;
  const qualityLabel = d.quality ? QUALITY_LABELS[d.quality] : undefined;

  return (
    <div
      className="rounded-xl border border-gray-700 bg-gray-900/95 px-3 py-2.5 text-xs shadow-xl backdrop-blur-sm"
      style={{ minWidth: 160 }}
    >
      <p className="mb-1 font-semibold text-white">
        {d.plyIndex === 0 ? "Starting position" : `Move ${Math.ceil(d.plyIndex / 2)}${d.plyIndex % 2 === 1 ? ". " : "… "}${d.san ?? ""}`}
      </p>
      <p className="text-gray-300">
        Eval:{" "}
        <span className={d.pawns >= 0 ? "text-teal-400" : "text-red-400"}>
          {evalLabel}
        </span>
      </p>
      {d.bestMove && (
        <p className="text-gray-400">
          Best: <span className="font-mono text-gray-200">{d.bestMove}</span>
        </p>
      )}
      {qualityLabel && (
        <p className="mt-1 font-semibold" style={{ color: qualityColor }}>
          {qualityLabel}
        </p>
      )}
    </div>
  );
}

// ── Legend ────────────────────────────────────────────────────────────────────

const LEGEND_ITEMS: Array<{ quality: MoveQuality; label: string }> = [
  { quality: "brilliant",  label: "Brilliant" },
  { quality: "inaccuracy", label: "Inaccuracy" },
  { quality: "mistake",    label: "Mistake" },
  { quality: "blunder",    label: "Blunder" },
];

function ChartLegend() {
  return (
    <div className="flex flex-wrap gap-x-4 gap-y-1 px-1 pt-1 pb-2">
      {LEGEND_ITEMS.map(({ quality, label }) => (
        <span key={quality} className="flex items-center gap-1.5 text-xs text-gray-400">
          <span
            className="inline-block h-2.5 w-2.5 rounded-full"
            style={{ backgroundColor: QUALITY_COLORS[quality] }}
          />
          {label}
        </span>
      ))}
    </div>
  );
}

// ── Main component ────────────────────────────────────────────────────────────

/**
 * CentipawnAdvantageChart
 *
 * Renders an interactive Recharts area chart of the engine evaluation across
 * every ply, with color-coded quality markers and click-to-jump scrubbing.
 *
 * @example
 *   <CentipawnAdvantageChart
 *     plies={evaluatedPlies}
 *     activePly={currentPly}
 *     onPlySelect={setPly}
 *   />
 */
export function CentipawnAdvantageChart({
  plies,
  activePly,
  onPlySelect,
  className = "",
}: CentipawnAdvantageChartProps) {
  // Build chart data: convert cp → pawns for the Y axis
  const data = useMemo(
    () =>
      plies.map((p) => ({
        ...p,
        pawns: cpToPawns(p.evaluation, p.mate),
      })),
    [plies]
  );

  const handleClick = useCallback(
    // recharts v3 reports the clicked point by index (no `activePayload`).
    (state: MouseHandlerDataParam) => {
      const index = Number(state?.activeTooltipIndex);
      const ply = Number.isInteger(index) ? data[index] : undefined;
      if (ply != null) onPlySelect(ply.plyIndex);
    },
    [data, onPlySelect]
  );

  if (plies.length === 0) {
    return (
      <div
        className={`flex items-center justify-center rounded-xl border border-gray-700 bg-gray-900 p-6 text-sm text-gray-500 ${className}`}
      >
        No evaluation data available
      </div>
    );
  }

  return (
    <div
      className={`rounded-xl border border-gray-700 bg-gray-900 p-4 ${className}`}
      data-testid="centipawn-advantage-chart"
    >
      <div className="mb-2 flex items-center justify-between">
        <h3 className="text-xs font-semibold uppercase tracking-wider text-gray-400">
          Evaluation Chart
        </h3>
        <ChartLegend />
      </div>

      <ResponsiveContainer width="100%" height={180}>
        <AreaChart
          data={data}
          margin={{ top: 8, right: 8, left: 0, bottom: 0 }}
          onClick={handleClick}
          style={{ cursor: "crosshair" }}
        >
          <defs>
            {/* Gradient above zero: white advantage */}
            <linearGradient id="evalGradientWhite" x1="0" y1="0" x2="0" y2="1">
              <stop offset="5%" stopColor="#f0f9ff" stopOpacity={0.25} />
              <stop offset="95%" stopColor="#f0f9ff" stopOpacity={0.02} />
            </linearGradient>
            {/* Gradient below zero: black advantage */}
            <linearGradient id="evalGradientBlack" x1="0" y1="1" x2="0" y2="0">
              <stop offset="5%" stopColor="#1e1b4b" stopOpacity={0.25} />
              <stop offset="95%" stopColor="#1e1b4b" stopOpacity={0.02} />
            </linearGradient>
          </defs>

          <CartesianGrid
            strokeDasharray="3 3"
            stroke="#374151"
            vertical={false}
          />

          {/* Zero reference line */}
          <ReferenceLine y={0} stroke="#6b7280" strokeWidth={1} />

          <XAxis
            dataKey="plyIndex"
            tickFormatter={plyToMoveLabel}
            tick={{ fill: "#9ca3af", fontSize: 10 }}
            tickLine={false}
            axisLine={{ stroke: "#374151" }}
            interval="preserveStartEnd"
          />

          <YAxis
            domain={Y_DOMAIN}
            tickFormatter={(v: number) => (v >= 0 ? `+${v}` : `${v}`)}
            tick={{ fill: "#9ca3af", fontSize: 10 }}
            tickLine={false}
            axisLine={false}
            width={32}
          />

          <Tooltip content={<ChartTooltip />} />

          {/* White-advantage area (positive values) */}
          <Area
            type="monotone"
            dataKey="pawns"
            stroke="#94a3b8"
            strokeWidth={1.5}
            fill="url(#evalGradientWhite)"
            isAnimationActive={false}
            dot={(props: DotItemDotProps) => (
              <QualityDot
                key={props.index}
                cx={Number(props.cx)}
                cy={Number(props.cy)}
                payload={props.payload as EvaluatedPly & { pawns: number }}
                activePly={activePly}
                onPlySelect={onPlySelect}
              />
            )}
            activeDot={false}
          />
        </AreaChart>
      </ResponsiveContainer>

      {/* Annotation counts */}
      <AnnotationSummary plies={plies} />
    </div>
  );
}

// ── Annotation summary bar ────────────────────────────────────────────────────

function AnnotationSummary({ plies }: { plies: EvaluatedPly[] }) {
  const counts = useMemo(() => {
    const c: Partial<Record<MoveQuality, number>> = {};
    for (const p of plies) {
      if (p.quality) c[p.quality] = (c[p.quality] ?? 0) + 1;
    }
    return c;
  }, [plies]);

  const tracked: MoveQuality[] = ["brilliant", "inaccuracy", "mistake", "blunder"];
  const hasAnnotations = tracked.some((q) => (counts[q] ?? 0) > 0);

  if (!hasAnnotations) return null;

  return (
    <div
      data-testid="annotation-summary"
      className="mt-2 flex flex-wrap gap-3 border-t border-gray-800 pt-2"
    >
      {tracked.map((q) =>
        (counts[q] ?? 0) > 0 ? (
          <span key={q} className="flex items-center gap-1 text-xs text-gray-400">
            <span
              className="inline-block h-2 w-2 rounded-full"
              style={{ backgroundColor: QUALITY_COLORS[q] }}
            />
            {counts[q]} {QUALITY_LABELS[q].split(" ")[0]}
          </span>
        ) : null
      )}
    </div>
  );
}

export default CentipawnAdvantageChart;
