"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { useParams } from "next/navigation";
import { EloRatingChart, type EloDataPoint } from "@/components/profile/EloRatingChart";
import { LoadingSkeleton } from "@/components/ui/LoadingSkeleton";
import { Button } from "@/components/ui/button";
import { useAuth } from "@/context/authContext";
import { useEloStats } from "@/hook/useEloStats";
import { truncateAddress } from "@/lib/utils";
import {
  PlayerNotFoundError,
  fetchEloHistory,
  fetchPlayerProfile,
  fetchRecentGames,
  playerHref,
  type PlayerProfile,
  type RecentGame,
} from "@/lib/playerProfileService";

const RESULT_STYLE: Record<RecentGame["result"], string> = {
  win: "text-emerald-300 bg-emerald-500/10 border-emerald-500/30",
  loss: "text-red-300 bg-red-500/10 border-red-500/30",
  draw: "text-gray-300 bg-gray-500/10 border-gray-500/30",
};

/**
 * Public player page: ratings, titles, rating history and recent games for
 * any player, so names in games and leaderboards can link somewhere.
 */
export default function PlayerPage() {
  const params = useParams<{ username: string }>();
  const username = decodeURIComponent(params?.username ?? "");
  const { user } = useAuth();

  const [profile, setProfile] = useState<PlayerProfile | null>(null);
  const [games, setGames] = useState<RecentGame[]>([]);
  const [eloData, setEloData] = useState<EloDataPoint[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notFound, setNotFound] = useState(false);
  const [reloadKey, setReloadKey] = useState(0);

  useEffect(() => {
    if (!username) return;
    const controller = new AbortController();
    setLoading(true);
    setError(null);
    setNotFound(false);

    Promise.allSettled([
      fetchPlayerProfile(username, controller.signal),
      fetchRecentGames(username, controller.signal),
      fetchEloHistory(username, controller.signal),
    ]).then(([p, g, h]) => {
      if (controller.signal.aborted) return;
      if (p.status === "fulfilled") {
        setProfile(p.value);
      } else if (p.reason instanceof PlayerNotFoundError) {
        setNotFound(true);
      } else {
        setProfile(null);
        setError(p.reason instanceof Error ? p.reason.message : "Could not load player.");
      }
      setGames(g.status === "fulfilled" ? g.value : []);
      setEloData(h.status === "fulfilled" ? h.value : []);
      setLoading(false);
    });
    return () => controller.abort();
  }, [username, reloadKey]);

  const stats = useEloStats(eloData, "all");
  const retry = useCallback(() => setReloadKey((k) => k + 1), []);
  const isMe = !!user && user.username.toLowerCase() === username.toLowerCase();

  if (loading) {
    return (
      <div className="mx-auto max-w-5xl space-y-6" role="status" aria-label="Loading player">
        <LoadingSkeleton className="h-10 w-64" />
        <LoadingSkeleton className="h-28 w-full rounded-2xl" />
        <LoadingSkeleton className="h-96 w-full rounded-2xl" />
      </div>
    );
  }

  if (notFound) {
    return (
      <div className="mx-auto max-w-xl py-16 text-center">
        <h1 className="text-2xl font-bold text-white">Player not found</h1>
        <p className="mt-2 text-sm text-gray-400">
          No player called <span className="font-mono">{username}</span> exists.
        </p>
        <Link href="/leaderboard" className="mt-4 inline-block text-sm text-teal-300 hover:underline">
          Browse the leaderboard
        </Link>
      </div>
    );
  }

  if (error || !profile) {
    return (
      <div className="mx-auto max-w-xl py-16 text-center" role="alert">
        <p className="text-sm text-red-300">{error ?? "Could not load player."}</p>
        <Button variant="outline" className="mt-4" onClick={retry}>
          Retry
        </Button>
      </div>
    );
  }

  const ratingEntries = Object.entries(profile.ratings);

  return (
    <div className="mx-auto max-w-5xl space-y-6" role="region" aria-label={`Player ${profile.username}`}>
      <header className="flex flex-wrap items-start justify-between gap-4">
        <div>
          <div className="flex flex-wrap items-center gap-2">
            {profile.titles.map((t) => (
              <span
                key={t}
                className="rounded bg-amber-500/20 px-2 py-0.5 text-xs font-bold text-amber-300 border border-amber-500/40"
                title={`Title: ${t}`}
              >
                {t}
              </span>
            ))}
            <h1 className="text-3xl font-bold text-white">{profile.username}</h1>
          </div>
          <p className="mt-1 text-sm text-gray-400">
            {profile.joinedAt && `Joined ${new Date(profile.joinedAt).toLocaleDateString()}`}
            {profile.joinedAt && profile.walletAddress && " · "}
            {profile.walletAddress && `wallet ${truncateAddress(profile.walletAddress)}`}
          </p>
        </div>
        {isMe && (
          <Link href="/profile" className="text-sm text-teal-300 hover:underline">
            Manage your profile
          </Link>
        )}
      </header>

      <section aria-label="Ratings" className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
        {ratingEntries.length === 0 ? (
          <div className="rounded-2xl border border-gray-700/30 bg-gray-800/40 p-5 sm:col-span-2 xl:col-span-4">
            <p className="text-sm text-gray-400">No rated games yet.</p>
          </div>
        ) : (
          ratingEntries.map(([tc, rating]) => (
            <div
              key={tc}
              className="rounded-2xl border border-gray-700/30 bg-gray-800/40 p-5 shadow-lg shadow-black/10"
            >
              <p className="text-xs uppercase tracking-[0.18em] text-gray-500">{tc}</p>
              <p className="mt-2 text-3xl font-bold text-white">{rating}</p>
            </div>
          ))
        )}
      </section>

      {eloData.length > 0 && (
        <section aria-label="Rating summary" className="grid gap-4 sm:grid-cols-3">
          {[
            { label: "Peak ELO", value: stats.peakElo },
            { label: "Games", value: stats.totalGames },
            { label: "Win rate", value: `${stats.winRate.toFixed(1)}%` },
          ].map((s) => (
            <div key={s.label} className="rounded-2xl border border-gray-700/30 bg-gray-800/40 p-4">
              <p className="text-xs uppercase tracking-[0.18em] text-gray-500">{s.label}</p>
              <p className="mt-1 text-2xl font-bold text-white">{s.value}</p>
            </div>
          ))}
        </section>
      )}

      <section aria-label="Rating history">
        {eloData.length === 0 ? (
          <div className="rounded-2xl border border-dashed border-gray-700 bg-gray-900/40 px-6 py-10 text-center">
            <p className="text-sm text-gray-400">No rating history yet.</p>
          </div>
        ) : (
          <EloRatingChart data={eloData} title={`${profile.username}'s ELO rating`} />
        )}
      </section>

      <section
        aria-labelledby="recent-games-heading"
        className="rounded-2xl border border-gray-700/50 bg-gray-900/60 p-4"
      >
        <h2 id="recent-games-heading" className="mb-3 text-lg font-semibold text-white">
          Recent games
        </h2>
        {games.length === 0 ? (
          <p className="text-sm text-gray-500">No recent games.</p>
        ) : (
          <ul className="divide-y divide-gray-700/40">
            {games.map((g) => (
              <li key={g.id} className="flex flex-wrap items-center gap-3 py-2 text-sm">
                <span className={`rounded border px-2 py-0.5 text-xs font-semibold capitalize ${RESULT_STYLE[g.result]}`}>
                  {g.result}
                </span>
                <span className="text-gray-300">
                  vs{" "}
                  <Link href={playerHref(g.opponent)} className="font-medium text-white hover:underline">
                    {g.opponent}
                  </Link>
                  {g.color && <span className="text-gray-500"> · as {g.color}</span>}
                  {g.timeControl && <span className="text-gray-500"> · {g.timeControl}</span>}
                </span>
                {typeof g.ratingChange === "number" && (
                  <span className={g.ratingChange >= 0 ? "text-emerald-300" : "text-red-300"}>
                    {g.ratingChange >= 0 ? "+" : ""}
                    {g.ratingChange}
                  </span>
                )}
                <span className="ml-auto flex items-center gap-3 text-xs text-gray-500">
                  {g.playedAt && new Date(g.playedAt).toLocaleDateString()}
                  <Link href={`/game/${encodeURIComponent(g.id)}/review`} className="text-teal-300 hover:underline">
                    Review
                  </Link>
                </span>
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}
