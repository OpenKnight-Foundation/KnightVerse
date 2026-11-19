"use client";

import { useCallback, useEffect, useState } from "react";
import { Loader2, Trophy } from "lucide-react";
import { LeaderboardTable } from "@/app/dashboard/LeaderboardTable";
import {
  fetchLeaderboard,
  type LeaderboardPage,
  type LeaderboardTimeControl,
} from "@/lib/leaderboardService";
import { useAuth } from "@/context/authContext";

const TIME_CONTROLS: { id: LeaderboardTimeControl; label: string }[] = [
  { id: "all", label: "Overall" },
  { id: "bullet", label: "Bullet" },
  { id: "blitz", label: "Blitz" },
  { id: "rapid", label: "Rapid" },
  { id: "classical", label: "Classical" },
];

const PAGE_SIZE = 50;

export default function LeaderboardPageRoute() {
  const { user } = useAuth();
  const [timeControl, setTimeControl] = useState<LeaderboardTimeControl>("all");
  const [page, setPage] = useState(1);
  const [data, setData] = useState<LeaderboardPage | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [reloadKey, setReloadKey] = useState(0);

  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    setError(null);
    fetchLeaderboard({ timeControl, page, pageSize: PAGE_SIZE, signal: controller.signal })
      .then((res) => {
        if (!controller.signal.aborted) setData(res);
      })
      .catch((err: unknown) => {
        if (controller.signal.aborted) return;
        setData(null);
        setError(err instanceof Error ? err.message : "Could not load the leaderboard.");
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [timeControl, page, reloadKey]);

  const retry = useCallback(() => setReloadKey((k) => k + 1), []);
  const totalPages = data ? Math.max(1, Math.ceil(data.total / PAGE_SIZE)) : 1;
  const username = user?.username ?? null;

  return (
    <div className="max-w-5xl mx-auto" role="region" aria-label="Leaderboard">
      <div className="flex items-center gap-3 mb-6">
        <Trophy className="h-8 w-8 text-yellow-400" aria-hidden="true" />
        <div>
          <h1 className="text-3xl font-bold text-white">Leaderboard</h1>
          <p className="text-sm text-gray-400">Top-rated KnightVerse players.</p>
        </div>
      </div>

      <div className="flex flex-wrap gap-2 mb-4" role="tablist" aria-label="Time control">
        {TIME_CONTROLS.map((tc) => (
          <button
            key={tc.id}
            role="tab"
            aria-selected={timeControl === tc.id}
            onClick={() => {
              setTimeControl(tc.id);
              setPage(1);
            }}
            className={`px-4 py-2 rounded-xl text-sm font-semibold border transition-colors ${
              timeControl === tc.id
                ? "bg-emerald-500/20 border-emerald-500/50 text-emerald-300"
                : "bg-gray-800/60 border-gray-700/50 text-gray-300 hover:text-white"
            }`}
          >
            {tc.label}
          </button>
        ))}
      </div>

      <div className="rounded-xl border border-gray-700/50 bg-gray-800/40 p-4" aria-busy={loading}>
        {loading && !data ? (
          <div className="flex items-center justify-center gap-2 py-12 text-gray-400" role="status">
            <Loader2 className="h-5 w-5 animate-spin" aria-hidden="true" />
            Loading leaderboard…
          </div>
        ) : error ? (
          <div className="py-10 text-center" role="alert">
            <p className="text-sm text-red-300 mb-3">{error}</p>
            <button
              onClick={retry}
              className="px-4 py-2 text-sm rounded-xl bg-gray-700/60 text-white hover:bg-gray-600/60"
            >
              Retry
            </button>
          </div>
        ) : (
          <>
            <LeaderboardTable rows={data?.rows ?? []} highlightPlayer={username} />
            {totalPages > 1 && (
              <nav className="flex items-center justify-between mt-4 text-sm" aria-label="Leaderboard pages">
                <button
                  onClick={() => setPage((p) => Math.max(1, p - 1))}
                  disabled={page <= 1 || loading}
                  className="px-3 py-1.5 rounded-lg bg-gray-700/60 text-gray-200 disabled:opacity-50"
                >
                  Previous
                </button>
                <span className="text-gray-400">
                  Page {page} of {totalPages}
                </span>
                <button
                  onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
                  disabled={page >= totalPages || loading}
                  className="px-3 py-1.5 rounded-lg bg-gray-700/60 text-gray-200 disabled:opacity-50"
                >
                  Next
                </button>
              </nav>
            )}
          </>
        )}
      </div>
    </div>
  );
}
