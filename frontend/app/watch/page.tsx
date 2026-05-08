"use client";

import { useEffect, useMemo, useState } from "react";
import { Eye, Radio, SearchX } from "lucide-react";
import { LiveGameCard } from "@/components/watch/LiveGameCard";
import { SpectatorBoard } from "@/components/watch/SpectatorBoard";
import {
  MOCK_LIVE_GAMES,
  type LiveGameMode,
  type LiveGameSummary,
  type MockLiveGameSummary,
} from "@/constants/mockLiveGames";
import { cn } from "@/lib/utils";

const API_BASE = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8000";
const FILTERS: Array<{ label: string; value: "all" | LiveGameMode }> = [
  { label: "All", value: "all" },
  { label: "Rated", value: "rated" },
  { label: "Casual", value: "casual" },
];

type LiveGameRecord = LiveGameSummary & {
  mode: LiveGameMode;
};

function normalizeLiveGame(game: Partial<LiveGameRecord>, index: number): LiveGameRecord | null {
  if (!game.gameId || !game.white?.address || !game.black?.address) {
    return null;
  }

  return {
    gameId: game.gameId,
    white: {
      address: game.white.address,
      elo: game.white.elo ?? 1200,
    },
    black: {
      address: game.black.address,
      elo: game.black.elo ?? 1200,
    },
    moveCount: game.moveCount ?? 0,
    spectatorCount: game.spectatorCount ?? 0,
    timeControl: game.timeControl ?? "10+0",
    status: game.status === "ending_soon" ? "ending_soon" : "playing",
    mode:
      game.mode ??
      ((game as { rated?: boolean }).rated === true
        ? "rated"
        : (game as { rated?: boolean }).rated === false
          ? "casual"
          : index % 2 === 0
            ? "rated"
            : "casual"),
  };
}

export default function WatchPage() {
  const [selectedGameId, setSelectedGameId] = useState<string | null>(null);
  const [filter, setFilter] = useState<"all" | LiveGameMode>("all");
  const [liveGames, setLiveGames] = useState<LiveGameRecord[]>(MOCK_LIVE_GAMES);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let active = true;

    const fetchLiveGames = async () => {
      try {
        const response = await fetch(`${API_BASE}/v1/games/live`, {
          credentials: "include",
        });

        if (!response.ok) {
          throw new Error(`HTTP ${response.status}`);
        }

        const payload = (await response.json()) as Array<Partial<LiveGameRecord>>;
        const normalizedGames = payload
          .map((game, index) => normalizeLiveGame(game, index))
          .filter((game): game is LiveGameRecord => game !== null);

        if (active) {
          setLiveGames(normalizedGames.length > 0 ? normalizedGames : MOCK_LIVE_GAMES);
        }
      } catch {
        if (active) {
          // Real API integration will replace this mock fallback when the live lobby endpoint is available.
          setLiveGames(MOCK_LIVE_GAMES as MockLiveGameSummary[]);
        }
      } finally {
        if (active) {
          setLoading(false);
        }
      }
    };

    fetchLiveGames();
    const intervalId = window.setInterval(fetchLiveGames, 10000);

    return () => {
      active = false;
      clearInterval(intervalId);
    };
  }, []);

  const filteredGames = useMemo(() => {
    if (filter === "all") {
      return liveGames;
    }

    return liveGames.filter((game) => game.mode === filter);
  }, [filter, liveGames]);

  const totalViewers = useMemo(
    () => liveGames.reduce((sum, game) => sum + game.spectatorCount, 0),
    [liveGames],
  );

  if (selectedGameId) {
    return <SpectatorBoard gameId={selectedGameId} onLeave={() => setSelectedGameId(null)} />;
  }

  return (
    <div className="space-y-6" role="region" aria-label="Live games lobby">
      <section className="animate-slide-up rounded-xl border border-gray-700/30 bg-gray-800/40 p-6">
        <div className="flex flex-col gap-6 lg:flex-row lg:items-center lg:justify-between">
          <div>
            <div className="inline-flex items-center gap-2 rounded-full border border-teal-500/20 bg-teal-500/10 px-3 py-1 text-xs uppercase tracking-[0.18em] text-teal-300">
              <Radio className="h-3.5 w-3.5" />
              Live spectator network
            </div>
            <h1 className="mt-4 text-3xl font-bold text-white md:text-4xl">Live Games</h1>
            <p className="mt-2 text-base text-gray-300">
              {totalViewers} viewers across {liveGames.length} active games
            </p>
          </div>

          <div className="flex flex-wrap gap-2" role="tablist" aria-label="Live game filters">
            {FILTERS.map((option) => (
              <button
                key={option.value}
                type="button"
                role="tab"
                aria-selected={filter === option.value}
                aria-label={`Show ${option.label.toLowerCase()} live games`}
                onClick={() => setFilter(option.value)}
                className={cn(
                  "rounded-full border px-4 py-2 text-sm font-semibold transition-all duration-300",
                  filter === option.value
                    ? "border-teal-400/40 bg-gradient-to-r from-teal-500 to-blue-700 text-white shadow-lg shadow-teal-900/30"
                    : "border-gray-700/50 bg-gray-900/50 text-gray-300 hover:border-gray-600 hover:bg-gray-800/70 hover:text-white",
                )}
              >
                {option.label}
              </button>
            ))}
          </div>
        </div>
      </section>

      {filteredGames.length === 0 && !loading ? (
        <div className="flex min-h-[320px] flex-col items-center justify-center rounded-xl border border-dashed border-gray-700/40 bg-gray-800/30 p-8 text-center animate-fade-in">
          <div className="flex h-16 w-16 items-center justify-center rounded-full bg-gradient-to-br from-teal-400/30 to-blue-500/30 text-teal-200">
            <SearchX className="h-8 w-8" />
          </div>
          <h2 className="mt-5 text-2xl font-semibold text-white">No live games right now</h2>
          <p className="mt-2 max-w-md text-sm text-gray-400">
            Check back in a few moments. New matches will appear here automatically.
          </p>
        </div>
      ) : (
        <section className="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-3" aria-label="Active live games">
          {filteredGames.map((game) => (
            <LiveGameCard
              key={game.gameId}
              gameId={game.gameId}
              white={game.white}
              black={game.black}
              moveCount={game.moveCount}
              spectatorCount={game.spectatorCount}
              timeControl={game.timeControl}
              status={game.status}
              onWatch={setSelectedGameId}
            />
          ))}
        </section>
      )}

      <div className="inline-flex items-center gap-2 rounded-full border border-gray-700/40 bg-gray-800/50 px-3 py-1 text-xs text-gray-400">
        <Eye className="h-3.5 w-3.5" />
        {loading ? "Refreshing live lobby…" : "Live list auto-refreshes every 10 seconds"}
      </div>
    </div>
  );
}
