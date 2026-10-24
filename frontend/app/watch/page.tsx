"use client";

import { useEffect, useMemo, useState } from "react";
import { Eye, Radio, SearchX } from "lucide-react";
import { LiveGameCard } from "@/components/watch/LiveGameCard";
import { SpectatorBoard } from "@/components/watch/SpectatorBoard";
import { PipWindowLayer } from "@/components/watch/PipWindowLayer";
import { usePipWindows } from "@/hook/usePipWindows";
import {
  type LiveGameMode,
  type LiveGameSummary,
  MOCK_LIVE_GAMES,
} from "@/constants/mockLiveGames";
import { cn } from "@/lib/utils";

const FILTERS: Array<{ label: string; value: "all" | LiveGameMode }> = [
  { label: "All", value: "all" },
  { label: "Rated", value: "rated" },
  { label: "Casual", value: "casual" },
];

type LiveGameRecord = LiveGameSummary & {
  mode: LiveGameMode;
};

const useMockLiveGames =
  process.env.NODE_ENV === "development" &&
  process.env.NEXT_PUBLIC_USE_MOCK_LIVE_GAMES === "true";

export default function WatchPage() {
  const [selectedGameId, setSelectedGameId] = useState<string | null>(null);
  const [filter, setFilter] = useState<"all" | LiveGameMode>("all");
  const [liveGames, setLiveGames] = useState<LiveGameRecord[]>(
    useMockLiveGames ? (MOCK_LIVE_GAMES as LiveGameRecord[]) : []
  );
  const [loading, setLoading] = useState(!useMockLiveGames);
  const [fetchError, setFetchError] = useState<string | null>(null);
  const {
    windows: pipWindows,
    viewport: pipViewport,
    isOpen: isPipOpen,
    open: openPip,
    close: closePip,
    focus: focusPip,
    setRect: setPipRect,
  } = usePipWindows();

  useEffect(() => {
    if (useMockLiveGames) return;

    // TODO(FE-101): wire up real fetch once GET /v1/games/live exists on the backend
    setLoading(false);
    setFetchError("Live games endpoint is not yet available.");
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

  const handleExpandPip = (gameId: string) => {
    closePip(gameId);
    setSelectedGameId(gameId);
  };

  if (selectedGameId) {
    return (
      <>
        <SpectatorBoard gameId={selectedGameId} onLeave={() => setSelectedGameId(null)} />
        <PipWindowLayer
          windows={pipWindows}
          viewport={pipViewport}
          activeGameId={selectedGameId}
          onFocus={focusPip}
          onClose={closePip}
          onExpand={handleExpandPip}
          onRectChange={setPipRect}
        />
      </>
    );
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

      {fetchError && (
        <div
          className="rounded-xl border border-red-500/30 bg-red-500/10 px-5 py-4 text-sm text-red-300 animate-fade-in"
          role="alert"
          aria-live="polite"
        >
          <p className="font-semibold">Unable to load live games</p>
          <p className="mt-0.5 text-red-400/80">{fetchError}</p>
        </div>
      )}

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
              onMinimize={openPip}
              isMinimized={isPipOpen(game.gameId)}
            />
          ))}
        </section>
      )}

      <div className="inline-flex items-center gap-2 rounded-full border border-gray-700/40 bg-gray-800/50 px-3 py-1 text-xs text-gray-400">
        <Eye className="h-3.5 w-3.5" />
        {loading
          ? "Refreshing live lobby…"
          : fetchError
          ? "Last refresh failed — retrying every 10 seconds"
          : "Live list auto-refreshes every 10 seconds"}
      </div>

      <PipWindowLayer
        windows={pipWindows}
        viewport={pipViewport}
        onFocus={focusPip}
        onClose={closePip}
        onExpand={handleExpandPip}
        onRectChange={setPipRect}
      />
    </div>
  );
}
