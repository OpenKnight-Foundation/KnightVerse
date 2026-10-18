import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  ReferenceLine,
  type TooltipContentProps,
} from 'recharts';

export interface EloDataPoint {
  date: string
  elo: number
  opponent: string
  change: number
}

// ── Custom tooltip ────────────────────────────────────────────────────────────

function CustomTooltip({ active, payload }: TooltipContentProps) {
  if (!active || !payload?.length) return null;

  const point = payload[0].payload as EloDataPoint;
  const isGain = point.change >= 0;

  return (
    <div className="rounded-lg border border-gray-200 bg-white px-4 py-3 shadow-lg text-sm">
      <p className="font-semibold text-gray-900 mb-1">
        {new Date(point.date).toLocaleDateString('en-US', {
          month: 'short',
          day: 'numeric',
          year: 'numeric',
        })}
      </p>
      <p className="text-gray-600">
        Rating: <span className="font-semibold text-gray-900">{point.elo}</span>
      </p>
      <p className={`font-semibold ${isGain ? 'text-emerald-600' : 'text-red-500'}`}>
        {isGain ? '+' : ''}{point.change} pts
      </p>
      <p className="text-gray-400 text-xs mt-1 truncate max-w-[180px]">
        vs {point.opponent}
      </p>
    </div>
  );
}

// ── Chart tick formatter ──────────────────────────────────────────────────────

function formatDateTick(dateStr: string): string {
  const d = new Date(dateStr);
  return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
}

// ── Main component ────────────────────────────────────────────────────────────

interface EloRatingChartProps {
  /**
   * Rating history to plot. Required since FE-131: the component used to fall
   * back to `MOCK_ELO_DATA`, which meant a profile page could silently render
   * fabricated ratings. Callers must supply real data (or their own empty
   * state).
   */
  data: EloDataPoint[]
  title?: string
}

export function EloRatingChart({
  data,
  title = 'Elo Rating — Last 30 Games',
}: EloRatingChartProps) {
  if (data.length === 0) {
    return (
      <div className="rounded-xl border border-gray-200 bg-white p-6 text-center text-sm text-gray-500">
        No rating history yet.
      </div>
    )
  }

  const elos = data.map((d) => d.elo)
  const minElo = Math.min(...elos)
  const maxElo = Math.max(...elos)
  const PADDING = 20
  const yDomain: [number, number] = [minElo - PADDING, maxElo + PADDING]
  const startingElo = data[0]?.elo ?? 1200

  const xTicks = data
    .filter((_, i) => i % 5 === 0 || i === data.length - 1)
    .map((d) => d.date);

  return (
    <div className="rounded-xl border border-gray-200 bg-white p-6">
      {/* Header */}
      <div className="flex items-center justify-between mb-6">
        <div>
          <h2 className="text-base font-semibold text-gray-900">{title}</h2>
          <p className="text-xs text-gray-400 mt-0.5">
            Current rating:{' '}
            <span className="font-semibold text-gray-700">
              {data[data.length - 1]?.elo ?? '—'}
            </span>
          </p>
        </div>

        {/* Net change badge */}
        {data.length > 1 && (() => {
          const net = (data[data.length - 1]?.elo ?? 0) - (data[0]?.elo ?? 0);
          const isPositive = net >= 0;
          return (
            <span
              className={`inline-flex items-center rounded-full px-3 py-1 text-sm font-semibold ${
                isPositive
                  ? 'bg-emerald-50 text-emerald-700'
                  : 'bg-red-50 text-red-600'
              }`}
            >
              {isPositive ? '+' : ''}{net} overall
            </span>
          );
        })()}
      </div>

      {/* Chart */}
      <ResponsiveContainer width="100%" height={280}>
        <LineChart
          data={data}
          margin={{ top: 8, right: 16, bottom: 4, left: 0 }}
        >
          <CartesianGrid strokeDasharray="3 3" stroke="#f0f0f0" />

          <XAxis
            dataKey="date"
            ticks={xTicks}
            tickFormatter={formatDateTick}
            tick={{ fontSize: 11, fill: '#9ca3af' }}
            axisLine={false}
            tickLine={false}
          />

          <YAxis
            domain={yDomain}
            tick={{ fontSize: 11, fill: '#9ca3af' }}
            axisLine={false}
            tickLine={false}
            width={40}
          />

          <Tooltip content={CustomTooltip} />

          {/* Reference line at the player's starting Elo */}
          <ReferenceLine
            y={startingElo}
            stroke="#e5e7eb"
            strokeDasharray="4 4"
            label={{
              value: `Start ${startingElo}`,
              position: 'insideTopRight',
              fontSize: 10,
              fill: '#d1d5db',
            }}
          />

          <Line
            type="monotone"
            dataKey="elo"
            stroke="#6366f1"
            strokeWidth={2}
            dot={{ r: 3, fill: '#6366f1', strokeWidth: 0 }}
            activeDot={{ r: 6, fill: '#6366f1', stroke: '#fff', strokeWidth: 2 }}
          />
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}
