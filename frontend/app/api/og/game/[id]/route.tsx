import { ImageResponse } from "next/og";
import type { NextRequest } from "next/server";

export const runtime = "edge";

interface GameResultSummary {
  whiteUsername: string;
  blackUsername: string;
  whiteRating: number;
  blackRating: number;
  resultText: string;
}

async function fetchPlayerInfo(
  apiBase: string,
  playerId?: string | null,
  fallbackName = "Player"
): Promise<{ username: string; rating: number }> {
  if (!playerId) {
    return { username: fallbackName, rating: 1500 };
  }
  try {
    const res = await fetch(`${apiBase}/v1/players/${encodeURIComponent(playerId)}`, {
      headers: { Accept: "application/json" },
      next: { revalidate: 300 },
    });
    if (res.ok) {
      const data = await res.json();
      const player = data?.data?.player || data?.player || data;
      if (player) {
        const username =
          player.username ||
          player.real_name ||
          `${fallbackName} ${playerId.slice(0, 6)}`;
        const rating =
          typeof player.rating === "number"
            ? player.rating
            : typeof player.fide_rating === "number"
            ? player.fide_rating
            : 1500;
        return { username, rating };
      }
    }
  } catch {
    // Return fallback on network error or offline mode
  }
  return { username: `${fallbackName} ${playerId.slice(0, 6)}`, rating: 1500 };
}

async function fetchGameSummary(gameId: string): Promise<GameResultSummary | null> {
  const trimmedId = gameId ? gameId.trim() : "";
  if (!trimmedId) return null;

  try {
    const apiBase =
      process.env.NEXT_PUBLIC_API_URL ||
      process.env.NEXT_PUBLIC_BACKEND_URL ||
      "http://localhost:8000";

    const res = await fetch(`${apiBase}/v1/games/${encodeURIComponent(trimmedId)}`, {
      headers: { Accept: "application/json" },
      next: { revalidate: 60 },
    });

    if (!res.ok) return null;
    const json = await res.json();
    const game = json?.data?.game || json?.game || json;
    if (!game || !game.id) return null;

    let whiteUsername: string = game.white_username || game.whiteUsername || "";
    let whiteRating: number = Number(game.white_rating ?? game.whiteRating ?? 0);
    let blackUsername: string = game.black_username || game.blackUsername || "";
    let blackRating: number = Number(game.black_rating ?? game.blackRating ?? 0);

    if (!whiteUsername || !blackUsername || !whiteRating || !blackRating) {
      const [whiteInfo, blackInfo] = await Promise.all([
        whiteUsername && whiteRating
          ? Promise.resolve({ username: whiteUsername, rating: whiteRating })
          : fetchPlayerInfo(apiBase, game.white_player_id || game.whitePlayerId, "White Player"),
        blackUsername && blackRating
          ? Promise.resolve({ username: blackUsername, rating: blackRating })
          : fetchPlayerInfo(apiBase, game.black_player_id || game.blackPlayerId, "Black Player"),
      ]);

      whiteUsername = whiteUsername || whiteInfo.username;
      whiteRating = whiteRating || whiteInfo.rating;
      blackUsername = blackUsername || blackInfo.username;
      blackRating = blackRating || blackInfo.rating;
    }

    let resultText = "Game in progress";
    const result = String(game.result || "").toLowerCase();
    const status = String(game.status || "").toLowerCase();

    if (result.includes("white") || result === "1-0") {
      resultText = `${whiteUsername} won`;
    } else if (result.includes("black") || result === "0-1") {
      resultText = `${blackUsername} won`;
    } else if (result.includes("draw") || result === "1/2-1/2") {
      resultText = "Game drawn";
    } else if (status === "waiting") {
      resultText = "Waiting for opponent";
    } else if (status === "aborted") {
      resultText = "Game aborted";
    } else if (status === "completed") {
      resultText = "Game concluded";
    }

    return {
      whiteUsername,
      blackUsername,
      whiteRating,
      blackRating,
      resultText,
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

  return new ImageResponse(
    (
      <div style={{ display: "flex", flexDirection: "column", width: "100%", height: "100%", background: "#0f172a", color: "#fff", padding: 48, fontSize: 32 }}>
        <div style={{ display: "flex", justifyContent: "space-between" }}>
          <span>{summary.whiteUsername} ({summary.whiteRating})</span>
          <span>vs</span>
          <span>{summary.blackUsername} ({summary.blackRating})</span>
        </div>
        <div style={{ marginTop: "auto", fontSize: 40 }}>{summary.resultText}</div>
      </div>
    ),
    { width: 1200, height: 630 }
  );
}
