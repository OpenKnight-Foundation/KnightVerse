"use client";

import { useCallback, useEffect, useState } from "react";
import { Coins, Loader2, Trophy } from "lucide-react";
import {
  claimSeasonPrize,
  fetchSeasonClaim,
  fetchSeasonStandings,
  fetchSeasonStatus,
  type SeasonClaim,
  type SeasonClaimResult,
  type SeasonStanding,
  type SeasonStatus,
} from "@/lib/seasonService";

const formatAmount = (value: number, currency: string) =>
  `${value.toLocaleString(undefined, { maximumFractionDigits: 2 })} ${currency}`;

export default function SeasonPage() {
  const [status, setStatus] = useState<SeasonStatus | null>(null);
  const [standings, setStandings] = useState<SeasonStanding[]>([]);
  const [claim, setClaim] = useState<SeasonClaim | null>(null);
  const [claimResult, setClaimResult] = useState<SeasonClaimResult | null>(null);
  const [loading, setLoading] = useState(true);
  const [claiming, setClaiming] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [claimError, setClaimError] = useState<string | null>(null);
  const [reloadKey, setReloadKey] = useState(0);

  const load = useCallback(async (signal: AbortSignal) => {
    const [nextStatus, nextStandings, nextClaim] = await Promise.all([
      fetchSeasonStatus(signal),
      fetchSeasonStandings(signal),
      fetchSeasonClaim(signal).catch(() => null),
    ]);
    if (signal.aborted) return;
    setStatus(nextStatus);
    setStandings(nextStandings);
    setClaim(nextClaim);
  }, []);

  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    setError(null);
    load(controller.signal)
      .catch((cause: unknown) => {
        if (controller.signal.aborted) return;
        setStatus(null);
        setError(cause instanceof Error ? cause.message : "Could not load season data.");
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [load, reloadKey]);

  const onClaim = async () => {
    if (!claim || claiming) return;
    setClaiming(true);
    setClaimError(null);
    try {
      const result = await claimSeasonPrize(claim);
      setClaimResult(result);
      if (result.claimed) {
        setClaim(null);
        setReloadKey((key) => key + 1);
      }
    } catch (cause) {
      setClaimError(cause instanceof Error ? cause.message : "Could not claim your prize.");
    } finally {
      setClaiming(false);
    }
  };

  return (
    <div className="max-w-5xl mx-auto" role="region" aria-label="Season prize pool">
      <div className="flex items-center gap-3 mb-6">
        <Trophy className="h-8 w-8 text-yellow-400" aria-hidden="true" />
        <div>
          <h1 className="text-3xl font-bold text-white">Season</h1>
          <p className="text-sm text-gray-400">{status?.name ?? "Loading season…"}</p>
        </div>
      </div>

      {loading && !status ? (
        <div className="flex items-center justify-center gap-2 rounded-xl border border-gray-700/50 bg-gray-800/40 p-12 text-gray-400" role="status">
          <Loader2 className="h-5 w-5 animate-spin" aria-hidden="true" />
          Loading season…
        </div>
      ) : error ? (
        <div className="rounded-xl border border-red-500/30 bg-red-500/10 p-6 text-center" role="alert">
          <p className="mb-3 text-sm text-red-300">{error}</p>
          <button
            type="button"
            onClick={() => setReloadKey((key) => key + 1)}
            className="rounded-xl bg-gray-700/60 px-4 py-2 text-sm text-white hover:bg-gray-600/60"
          >
            Retry
          </button>
        </div>
      ) : (
        <div className="space-y-6">
          <section
            className="rounded-2xl border border-yellow-500/30 bg-gradient-to-r from-yellow-500/10 to-amber-600/5 p-6"
            aria-labelledby="prize-pool-title"
          >
            <div className="flex flex-wrap items-center justify-between gap-4">
              <div className="flex items-center gap-4">
                <Coins className="h-10 w-10 text-yellow-400" aria-hidden="true" />
                <div>
                  <h2 id="prize-pool-title" className="text-sm font-semibold uppercase tracking-widest text-yellow-300">
                    Prize pool
                  </h2>
                  <p className="mt-1 text-3xl font-bold text-white">
                    {status ? formatAmount(status.prizePool, status.currency) : "—"}
                  </p>
                </div>
              </div>
              <p
                className={`rounded-xl px-4 py-2 text-sm font-semibold ${
                  status?.phase === "ended"
                    ? "bg-emerald-500/15 text-emerald-300"
                    : "bg-teal-500/15 text-teal-300"
                }`}
                role="status"
              >
                {status?.phase === "ended" ? "Season ended — claims open" : "Season in progress"}
              </p>
            </div>
            {status?.claimDeadline && (
              <p className="mt-4 text-xs text-gray-400">
                Claim deadline: {new Date(status.claimDeadline).toLocaleString()}
              </p>
            )}
          </section>

          <section className="rounded-xl border border-gray-700/50 bg-gray-800/40 p-4" aria-labelledby="standings-title">
            <h2 id="standings-title" className="mb-4 text-sm font-semibold text-gray-300">
              Live standings
            </h2>
            {standings.length === 0 ? (
              <p className="py-8 text-center text-sm text-gray-500">No standings published yet.</p>
            ) : (
              <div className="overflow-x-auto">
                <table className="w-full text-sm">
                  <caption className="sr-only">Season standings</caption>
                  <thead className="text-gray-400">
                    <tr>
                      <th scope="col" className="p-2 text-left">Rank</th>
                      <th scope="col" className="p-2 text-left">Player</th>
                      <th scope="col" className="p-2 text-right">Points</th>
                      <th scope="col" className="p-2 text-right">Prize</th>
                    </tr>
                  </thead>
                  <tbody>
                    {standings.map((row) => (
                      <tr key={`${row.rank}-${row.player}`} className="border-t border-gray-700/40">
                        <td className="p-2 text-gray-400">#{row.rank}</td>
                        <td className="p-2 font-medium text-white">{row.player}</td>
                        <td className="p-2 text-right text-gray-200">{row.points.toLocaleString()}</td>
                        <td className="p-2 text-right text-emerald-300">
                          {row.prize === undefined ? "—" : formatAmount(row.prize, status?.currency ?? "XLM")}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </section>

          {status?.phase === "ended" && (
            <section
              className="rounded-xl border border-teal-500/30 bg-teal-500/10 p-6"
              aria-labelledby="claim-title"
              aria-busy={claiming}
            >
              <h2 id="claim-title" className="text-lg font-bold text-white">Claim your prize</h2>
              {claimResult?.claimed ? (
                <p className="mt-3 text-sm text-emerald-300" role="status">
                  Claimed {formatAmount(claimResult.amount, status.currency)}.
                  {claimResult.transactionHash ? ` Transaction ${claimResult.transactionHash}.` : ""}
                </p>
              ) : (
                <>
                  <p className="mt-2 text-sm text-gray-300">
                    {claim
                      ? `Your verified allocation is ${formatAmount(claim.amount, claim.currency)}. The Merkle proof was supplied by the backend.`
                      : "No claimable prize is available for this account."}
                  </p>
                  {claimError && (
                    <p className="mt-3 text-sm text-red-300" role="alert">{claimError}</p>
                  )}
                  <button
                    type="button"
                    onClick={() => void onClaim()}
                    disabled={!claim || claiming}
                    className="mt-4 rounded-xl bg-teal-500 px-5 py-3 text-sm font-bold text-black transition-all hover:bg-teal-400 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-teal-300 disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    {claiming ? "Claiming…" : "Claim prize"}
                  </button>
                </>
              )}
            </section>
          )}
        </div>
      )}
    </div>
  );
}
