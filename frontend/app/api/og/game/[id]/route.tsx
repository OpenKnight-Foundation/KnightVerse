import { ImageResponse } from "next/og";
import type { NextRequest } from "next/server";

export const runtime = "edge";

interface GameResultSummary {
  whiteUsername: string;
  blackUsername: string;
  whiteRating: number | null;
  blackRating: number | null;
  resultText: string;
}

/** Maps the backend GameResult enum value to a human-readable string. */
function formatResult(result: string, status: string): string {
  if (status === "in_progress" || status === "waiting") return "Game in progress";
  if (status === "aborted") return "Game aborted";
  switch (result) {
    case "white_win": return "White wins";
    case "black_win": return "Black wins";
    case "draw":      return "Draw";
    default:          return "Game over";
  }
}

/**
 * Fetches a single player's username by their UUID.
 * Returns null on any error so callers can fall back gracefully.
 */
async function fetchUsername(apiBase: string, playerId: string): Promise<string | null> {
  try {
    const res = await fetch(`${apiBase}/v1/players/${playerId}`, {
      next: { revalidate: 60 },
    });
    if (!res.ok) return null;
    const json = await res.json() as { data?: { player?: { username?: string } } };
    return json?.data?.player?.username ?? null;
  } catch {
    return null;
  }
}

/**
 * Fetches real game data from the backend and returns a summary suitable for
 * rendering in the OG card, or null when the game is not found / on error.
 */
async function fetchGameSummary(gameId: string): Promise<GameResultSummary | null> {
  if (!gameId) return null;

  // NEXT_PUBLIC_API_URL is inlined by Next.js at build time for edge functions.
  const apiBase =
    process.env.NEXT_PUBLIC_API_URL?.replace(/\/$/, "") ?? "http://localhost:8080";

  try {
    const res = await fetch(`${apiBase}/v1/games/${gameId}`, {
      next: { revalidate: 30 },
    });

    // Surface "not found" explicitly so the caller can render the fallback card.
    if (res.status === 404) return null;
    if (!res.ok) return null;

    const json = await res.json() as {
      data?: {
        game?: {
          white_player_id?: string;
          black_player_id?: string | null;
          status?: string;
          result?: string;
        };
      };
    };

    const game = json?.data?.game;
    if (!game) return null;

    const { white_player_id, black_player_id, status = "unknown", result = "in_progress" } = game;

    // Resolve usernames in parallel; fall back to a generic label on failure.
    const [whiteUsername, blackUsername] = await Promise.all([
      white_player_id ? fetchUsername(apiBase, white_player_id) : Promise.resolve(null),
      black_player_id ? fetchUsername(apiBase, black_player_id) : Promise.resolve(null),
    ]);

    return {
      whiteUsername: whiteUsername ?? "White Player",
      blackUsername: blackUsername ?? "Waiting for opponent",
      // The backend's GET /v1/players/{id} (DisplayPlayer) does not expose
      // elo_rating yet. Ratings are shown as null until the endpoint includes
      // the field, so the card omits the number rather than hard-coding 1500.
      whiteRating: null,
      blackRating: null,
      resultText: formatResult(result, status),
    };
  } catch {
    return null;
  }
}

/** Generates a 1200x630 OpenGraph share card for a finished/ongoing game. */
export async function GET(
  _req: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  const { id } = await params;
  const summary = await fetchGameSummary(id);

  if (!summary) {
    return new ImageResponse(
      (
        <div style={{ display: "flex", fontSize: 48, background: "#111", color: "#fff", width: "100%", height: "100%", alignItems: "center", justifyContent: "center" }}>
          KnightVerse
        </div>
      ),
      { width: 1200, height: 630 }
    );
  }

  const whiteLabel = summary.whiteRating !== null
    ? `${summary.whiteUsername} (${summary.whiteRating})`
    : summary.whiteUsername;
  const blackLabel = summary.blackRating !== null
    ? `${summary.blackUsername} (${summary.blackRating})`
    : summary.blackUsername;

  return new ImageResponse(
    (
      <div style={{ display: "flex", flexDirection: "column", width: "100%", height: "100%", background: "#0f172a", color: "#fff", padding: 48, fontSize: 32 }}>
        <div style={{ display: "flex", justifyContent: "space-between" }}>
          <span>{whiteLabel}</span>
          <span>vs</span>
          <span>{blackLabel}</span>
        </div>
        <div style={{ marginTop: "auto", fontSize: 40 }}>{summary.resultText}</div>
      </div>
    ),
    { width: 1200, height: 630 }
  );
}
