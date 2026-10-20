"use client";

import { useCallback, useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { EloRatingChart, type EloDataPoint } from "@/components/profile/EloRatingChart";
import { SessionManager } from "@/components/profile/SessionManager";
import LinkedWalletSettings from "@/components/LinkedWalletSettings";
import { GameSidebar } from "@/components/GameSidebar";
import { Header } from "@/components/Header";
import { Button } from "@/components/ui/button";
import { LoadingSkeleton } from "@/components/ui/LoadingSkeleton";
import { useAuth } from "@/context/authContext";
import { useEloStats } from "@/hook/useEloStats";
import { endpoints } from "@/lib/api";
import { truncateAddress } from "@/lib/utils";

/**
 * Player profile (FE-131 / #1372).
 *
 * Mounts the two orphaned profile components — `SessionManager` and
 * `EloRatingChart` — behind a sign-in gate, and adds the `/profile` route the
 * sidebar links to.
 *
 * Rating history comes from `GET /v1/players/{id}/elo-history`. Like the
 * dashboard, a 404 degrades to "no history yet" instead of surfacing an error,
 * because the backend keys players by UUID / wallet address rather than the
 * integer `user_id` (#FE-103).
 */
export default function ProfilePage() {
  const { accessToken, user, isAuthenticated, isLoading: authLoading } = useAuth();
  const router = useRouter();
  const [collapsed, setCollapsed] = useState(false);

  const [eloData, setEloData] = useState<EloDataPoint[]>([]);
  const [historyLoading, setHistoryLoading] = useState(false);
  const [historyError, setHistoryError] = useState<string | null>(null);
  /** Bumped by the Refresh button to re-run the history fetch. */
  const [refreshKey, setRefreshKey] = useState(0);

  // The backend keys players by UUID / wallet address, not `user_id`.
  const playerId = user?.wallet_address ?? user?.username ?? null;

  // Profile is sign-in only: bounce anonymous visitors back to the landing
  // page instead of rendering an empty shell.
  useEffect(() => {
    if (!authLoading && !isAuthenticated) {
      router.replace("/");
    }
  }, [authLoading, isAuthenticated, router]);

  useEffect(() => {
    if (authLoading || !isAuthenticated || !accessToken || !playerId) return;

    let cancelled = false;
    setHistoryLoading(true);
    setHistoryError(null);

    fetch(endpoints.players.eloHistory(playerId), {
      headers: { Authorization: `Bearer ${accessToken}` },
    })
      .then(async (res) => {
        // Endpoint not live yet — degrade gracefully (#FE-103).
        if (res.status === 404) return [] as EloDataPoint[];
        if (!res.ok) throw new Error(`Failed to load rating history (${res.status})`);
        return res.json() as Promise<EloDataPoint[]>;
      })
      .then((data) => {
        if (!cancelled) setEloData(data);
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          setHistoryError(err instanceof Error ? err.message : "Unknown error");
        }
      })
      .finally(() => {
        if (!cancelled) setHistoryLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [accessToken, authLoading, isAuthenticated, playerId, refreshKey]);

  const stats = useEloStats(eloData, "all");

  const handleRefresh = useCallback(() => {
    setRefreshKey((key) => key + 1);
  }, []);

  if (authLoading) {
    return (
      <div className="flex h-screen overflow-hidden bg-background">
        <GameSidebar collapsed={collapsed} setCollapsed={setCollapsed} />
        <div className={`flex-1 transition-all duration-500 ${collapsed ? "md:ml-16" : "md:ml-64"}`}>
          <Header />
          <main className="flex-1 overflow-y-auto p-4 md:p-8" role="status" aria-label="Loading profile">
            <LoadingSkeleton className="h-10 w-64 mb-8" />
            <LoadingSkeleton className="h-64 w-full rounded-2xl mb-6" />
            <LoadingSkeleton className="h-48 w-full rounded-2xl" />
          </main>
        </div>
      </div>
    );
  }

  if (!isAuthenticated) return null;

  return (
    <div className="flex h-screen overflow-hidden bg-background">
      <GameSidebar collapsed={collapsed} setCollapsed={setCollapsed} />

      <div
        className={`flex-1 flex flex-col transition-all duration-500 ease-in-out ${
          collapsed ? "md:ml-16" : "md:ml-64"
        }`}
      >
        <Header />

        <main className="flex-1 overflow-y-auto p-4 md:p-8">
          <div className="mx-auto max-w-5xl space-y-6">
            <header className="flex flex-wrap items-start justify-between gap-4">
              <div>
                <h1 className="text-3xl font-bold text-white">{user?.username}</h1>
                <p className="mt-1 text-sm text-gray-400">
                  Player #{user?.user_id} ·{" "}
                  {user?.wallet_address
                    ? `wallet ${truncateAddress(user.wallet_address)}`
                    : "no wallet linked"}
                </p>
              </div>
              <Button variant="outline" onClick={handleRefresh} disabled={historyLoading}>
                {historyLoading ? "Refreshing…" : "Refresh"}
              </Button>
            </header>

            <section
              aria-label="Player summary"
              className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4"
            >
              {[
                { label: "Current ELO", value: eloData.length ? stats.currentElo : "—" },
                { label: "Peak ELO", value: eloData.length ? stats.peakElo : "—" },
                { label: "Games", value: eloData.length ? stats.totalGames : "—" },
                {
                  label: "Win rate",
                  value: eloData.length ? `${stats.winRate.toFixed(1)}%` : "—",
                },
              ].map((stat) => (
                <div
                  key={stat.label}
                  className="rounded-2xl border border-gray-700/30 bg-gray-800/40 p-5 shadow-lg shadow-black/10"
                >
                  <p className="text-xs uppercase tracking-[0.18em] text-gray-500">{stat.label}</p>
                  <p className="mt-2 text-3xl font-bold text-white">{stat.value}</p>
                </div>
              ))}
            </section>

            {historyError && (
              <p role="alert" className="text-sm text-red-400">
                {historyError}
              </p>
            )}

            <section aria-label="Rating history">
              {historyLoading ? (
                <LoadingSkeleton className="h-96 w-full rounded-2xl" />
              ) : eloData.length === 0 ? (
                <div className="rounded-2xl border border-dashed border-gray-700 bg-gray-900/40 px-6 py-12 text-center">
                  <h2 className="text-lg font-semibold text-white">No rating history yet</h2>
                  <p className="mt-2 text-sm text-gray-400">
                    Play a rated match to start building your ELO chart.
                  </p>
                </div>
              ) : (
                <EloRatingChart data={eloData} title="Your ELO rating" />
              )}
            </section>

            <div className="grid gap-6 lg:grid-cols-2">
              <LinkedWalletSettings />
              <section
                aria-label="Active sessions"
                className="rounded-2xl border border-gray-700/50 bg-gray-900/60 p-6"
              >
                <SessionManager />
              </section>
            </div>
          </div>
        </main>
      </div>
    </div>
  );
}