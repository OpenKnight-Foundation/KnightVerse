"use client";

import {
  Area,
  AreaChart,
  Brush,
  CartesianGrid,
  ReferenceLine,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
  type TooltipContentProps,
} from "recharts";
import type {
  NameType,
  ValueType,
} from "recharts/types/component/DefaultTooltipContent";
import type { EloDataPoint } from "@/components/profile/EloRatingChart";

interface EloChartProps {
  /** One entry per game, in chronological order (oldest first). */
  data: EloDataPoint[];
  /** Heading shown above the chart. */
  title?: string;
  /** Chart height in pixels. */
  height?: number;
}

/** Short "Mar 5" label used for X-axis ticks and the Brush slider. */
function formatDateTick(date: string): string {
  return new Date(date).toLocaleDateString("en-US", {
    month: "short",
    day: "numeric",
  });
}

/**
 * Custom hover tooltip: shows the date, rating after the game, the rating
 * change (green for gains, red for losses) and the opponent.
 * Recharts supplies `active` (is the user hovering?) and `payload` (the data
 * under the cursor).
 */
function CustomTooltip({ active, payload }: TooltipContentProps<ValueType, NameType>) {
  // Render nothing unless the user is hovering over a data point
  if (!active || !payload?.length) {
    return null;
  }

  // The full data point for the hovered game is on the first payload entry
  const point = payload[0]?.payload as EloDataPoint;
  // A zero change counts as positive, so draws display as "+0 pts" in green
  const positive = point.change >= 0;

  return (
    <div className="min-w-44 rounded-xl border border-gray-700 bg-gray-800/95 px-4 py-3 shadow-2xl shadow-black/30 backdrop-blur-sm">
      <p className="text-sm font-semibold text-white">
        {new Date(point.date).toLocaleDateString("en-US", {
          month: "short",
          day: "numeric",
          year: "numeric",
        })}
      </p>
      <div className="mt-2 space-y-1 text-sm">
        <p className="text-gray-300">
          Rating <span className="font-semibold text-white">{point.elo}</span>
        </p>
        {/* Negative numbers already include "-", so only add "+" for non-negative values */}
        <p className={positive ? "font-semibold text-emerald-400" : "font-semibold text-red-400"}>
          {positive ? "+" : ""}{point.change} pts
        </p>
        {/* `truncate` keeps very long opponent names on one line */}
        <p className="truncate text-xs text-gray-500">vs {point.opponent}</p>
      </div>
    </div>
  );
}

/**
 * Area chart of a player's rating over time, with a start-rating reference
 * line, a net-change badge, and a zoom slider (Brush) for long histories.
 * Shows a shimmering skeleton while `data` is empty.
 */
export default function EloChart({
  data,
  title = "Rating Momentum",
  height = 360,
}: EloChartProps) {
  // Empty data is treated as "still loading": render a skeleton that
  // roughly matches the layout of the real chart to avoid layout shift.
  if (!data.length) {
    return (
      <section
        role="region"
        aria-label="ELO chart loading"
        className="rounded-xl border border-gray-700/30 bg-gray-800/40 p-6"
      >
        {/* Skeleton for the title, subtitle and badge row */}
        <div className="mb-6 flex items-center justify-between gap-4">
          <div className="space-y-3">
            <div className="h-4 w-36 rounded-full bg-gray-700 shimmer-bg animate-shimmer" />
            <div className="h-3 w-28 rounded-full bg-gray-700 shimmer-bg animate-shimmer" />
          </div>
          <div className="h-8 w-24 rounded-full bg-gray-700 shimmer-bg animate-shimmer" />
        </div>
        {/* Skeleton for the chart area */}
        <div className="h-72 rounded-2xl bg-gray-900/40 shimmer-bg animate-shimmer" />
      </section>
    );
  }

  // ── Derived values (safe to compute here because `data` is non-empty) ──
  const currentElo = data[data.length - 1]?.elo ?? 0; // latest rating
  const startingElo = data[0]?.elo ?? 0; // rating at the beginning of the sample
  const yValues = data.map((point) => point.elo);
  const minElo = Math.min(...yValues);
  const maxElo = Math.max(...yValues);
  // Vertical padding for the Y axis: 18% of the rating range, but at least
  // 18 points, so flat histories don't render as a squashed line.
  const padding = Math.max(18, Math.round((maxElo - minElo) * 0.18));
  // Aim for roughly 6 X-axis labels regardless of how many games there are
  const xTickStep = Math.max(1, Math.floor(data.length / 6));
  // Every Nth date, plus the final one so the latest game is always labelled
  const xTicks = data.filter((_, index) => index % xTickStep === 0 || index === data.length - 1).map((point) => point.date);
  // Total rating gained (positive) or lost (negative) over the sample
  const netChange = currentElo - startingElo;

  return (
    <section
      role="region"
      aria-label="ELO progress chart"
      className="rounded-xl border border-gray-700/30 bg-gray-800/40 p-6 shadow-lg shadow-black/10"
    >
      {/* Header: title and description on the left, net-change badge on the right */}
      <div className="mb-6 flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
        <div>
          <h2 className="text-lg font-semibold text-white">{title}</h2>
          <p className="mt-1 text-sm text-gray-400">
            Interactive rating history across the selected sample.
          </p>
        </div>
        {/* Badge is green for a net gain (or zero), red for a net loss */}
        <div
          className={netChange >= 0
            ? "inline-flex items-center rounded-full border border-emerald-500/30 bg-emerald-500/10 px-3 py-1 text-sm font-semibold text-emerald-400"
            : "inline-flex items-center rounded-full border border-red-500/30 bg-red-500/10 px-3 py-1 text-sm font-semibold text-red-400"}
        >
          {netChange >= 0 ? "+" : ""}{netChange} overall
        </div>
      </div>

      <ResponsiveContainer width="100%" height={height}>
        <AreaChart data={data} margin={{ top: 12, right: 18, left: 0, bottom: 12 }}>
          {/* Reusable SVG gradients, referenced below by id via url(#...) */}
          <defs>
            {/* Vertical fill under the line: indigo at the top fading to transparent */}
            <linearGradient id="eloAreaGradient" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor="#6366f1" stopOpacity={0.45} />
              <stop offset="55%" stopColor="#2dd4bf" stopOpacity={0.18} />
              <stop offset="100%" stopColor="#0f172a" stopOpacity={0} />
            </linearGradient>
            {/* Horizontal gradient for the line itself: indigo (left) to teal (right) */}
            <linearGradient id="eloStrokeGradient" x1="0" y1="0" x2="1" y2="0">
              <stop offset="0%" stopColor="#6366f1" />
              <stop offset="100%" stopColor="#14b8a6" />
            </linearGradient>
          </defs>

          {/* Faint dashed horizontal grid lines only */}
          <CartesianGrid stroke="rgba(148, 163, 184, 0.14)" strokeDasharray="3 3" vertical={false} />
          <XAxis
            dataKey="date"
            // Use the precomputed, evenly spread tick dates
            ticks={xTicks}
            tickFormatter={formatDateTick}
            tick={{ fill: "#94a3b8", fontSize: 12 }}
            axisLine={false}
            tickLine={false}
            // Drop labels that would be closer than 20px to a neighbour
            minTickGap={20}
          />
          <YAxis
            // Padded range so the line never touches the top or bottom edge
            domain={[minElo - padding, maxElo + padding]}
            tick={{ fill: "#94a3b8", fontSize: 12 }}
            axisLine={false}
            tickLine={false}
            width={44}
          />
          {/* Uses CustomTooltip above; `cursor` styles the vertical hover line */}
          <Tooltip content={CustomTooltip} cursor={{ stroke: "rgba(45, 212, 191, 0.3)", strokeWidth: 1.5 }} />
          {/* Dashed horizontal line at the starting rating, to show progress against it */}
          <ReferenceLine
            y={startingElo}
            stroke="rgba(45, 212, 191, 0.35)"
            strokeDasharray="5 5"
            // Expand the Y domain if the line would otherwise fall outside it
            ifOverflow="extendDomain"
            label={{
              value: `Start ${startingElo}`,
              position: "insideTopRight",
              fill: "#5eead4",
              fontSize: 11,
            }}
          />
          {/* The rating line and its gradient-filled area */}
          <Area
            type="monotone" // smooth curve between points
            dataKey="elo"
            stroke="url(#eloStrokeGradient)"
            fill="url(#eloAreaGradient)"
            strokeWidth={3}
            // Hide dots on the line; show a larger dot only on hover
            dot={{ r: 0 }}
            activeDot={{ r: 6, fill: "#14b8a6", stroke: "#0f172a", strokeWidth: 2 }}
            animationDuration={900}
          />
          {/* Zoom/pan slider, shown only for longer histories (more than 18 games)
              where the chart would otherwise get crowded */}
          {data.length > 18 ? (
            <Brush
              dataKey="date"
              height={26}
              stroke="#14b8a6"
              travellerWidth={10}
              fill="rgba(15, 23, 42, 0.85)"
              tickFormatter={formatDateTick}
            />
          ) : null}
        </AreaChart>
      </ResponsiveContainer>
    </section>
  );
}